mod dithering;
pub mod pixels;
mod rice;

use dithering::{N_RANDOM, RAND_VALUES};

use crate::hdu::header::extension::bintable::{BinTable, TileCompressedImage, ZQuantiz};
use crate::hdu::header::Header;

pub trait Keywords {
    type T;

    fn new(header: &Header<BinTable>, config: &TileCompressedImage) -> Self;
}

#[derive(Debug)]
struct TileDesc<K> {
    /// Total number of pixels in the tile
    pub n_pixels: u64,
    /// Number of pixels remaining to return
    pub remaining_pixels: u64,
    /// Optional keywords relative to the parsing of floats
    pub keywords: K,
}

impl<K> TileDesc<K>
where
    K: Keywords,
{
    fn new(header: &Header<BinTable>, config: &TileCompressedImage) -> Self {
        let keywords = K::new(header, config);

        Self {
            keywords,
            n_pixels: 0,
            remaining_pixels: 0,
        }
    }
}

#[derive(Debug)]
pub struct F32Keywords {
    /// Idx column storing z_scale values for each tile
    z_scale_idx: Option<usize>,
    /// Idx column storing z_zero values for each tile
    z_zero_idx: Option<usize>,
    /// Idx column storing z_blank values for each tile
    z_blank_idx: Option<usize>,

    /// Dither
    z_dither_0: i64,

    /// Current value of ZSCALE field (floating point case)
    scale: f32,
    /// Current value of ZZERO field (floating point case)
    zero: f32,
    /// Current value of ZBLANK (floating point case)
    /// Stores the integer value that evaluates to a floating point NAN
    z_blank: Option<i32>,
    /// Current value of ZBLANK field (float and integer case)
    /// Current quantiz state
    quantiz: Option<Quantiz>,
}

#[derive(Debug)]
pub struct F64Keywords {
    /// Idx column storing z_scale values for each tile
    z_scale_idx: Option<usize>,
    /// Idx column storing z_zero values for each tile
    z_zero_idx: Option<usize>,
    /// Idx column storing z_blank values for each tile
    z_blank_idx: Option<usize>,

    /// Dither
    z_dither_0: i64,

    /// Current value of ZSCALE field (floating point case)
    scale: f32,
    /// Current value of ZZERO field (floating point case)
    zero: f32,
    /// Current value of ZBLANK (floating point case)
    /// Stores the integer value that evaluates to a floating point NAN
    z_blank: Option<i32>,
    /// Current value of ZBLANK field (float and integer case)
    /// Current quantiz state
    quantiz: Option<Quantiz>,
}

impl F64Keywords {
    /// Unquantize the integer decoded value to the real floating point value
    fn unquantize(&mut self, value: i32) -> f32 {
        // map the NaN if value corresponds to BLANK
        if let Some(z_blank) = self.z_blank {
            if z_blank == value {
                return f32::NAN;
            }
        }

        if let Some(q) = &mut self.quantiz {
            let value = match q {
                Quantiz::NoDither => value as f32,
                Quantiz::SubtractiveDither1 { i1 } => {
                    let ri = RAND_VALUES[*i1];
                    // increment i1 for the next pixel
                    *i1 = (*i1 + 1) % N_RANDOM;

                    (value as f32) - ri + 0.5
                }
                Quantiz::SubtractiveDither2 { i1 } => {
                    // FIXME: i32::MIN is -2147483648 !
                    if value == -2147483647 {
                        *i1 = (*i1 + 1) % N_RANDOM;

                        0.0
                    } else {
                        let ri = RAND_VALUES[*i1];
                        // increment i1 for the next pixel
                        *i1 = (*i1 + 1) % N_RANDOM;

                        (value as f32) - ri + 0.5
                    }
                }
            };

            value * self.scale + self.zero
        } else {
            f32::from_bits(value as u32)
        }
    }
}

impl F32Keywords {
    /// Unquantize the integer decoded value to the real floating point value
    fn unquantize(&mut self, value: i32) -> f32 {
        // map the NaN if value corresponds to BLANK
        if let Some(z_blank) = self.z_blank {
            if z_blank == value {
                return f32::NAN;
            }
        }

        if let Some(q) = &mut self.quantiz {
            let value = match q {
                Quantiz::NoDither => value as f32,
                Quantiz::SubtractiveDither1 { i1 } => {
                    let ri = RAND_VALUES[*i1];
                    // increment i1 for the next pixel
                    *i1 = (*i1 + 1) % N_RANDOM;

                    (value as f32) - ri + 0.5
                }
                Quantiz::SubtractiveDither2 { i1 } => {
                    // FIXME: i32::MIN is -2147483648 !
                    if value == -2147483647 {
                        *i1 = (*i1 + 1) % N_RANDOM;

                        0.0
                    } else {
                        let ri = RAND_VALUES[*i1];
                        // increment i1 for the next pixel
                        *i1 = (*i1 + 1) % N_RANDOM;

                        (value as f32) - ri + 0.5
                    }
                }
            };

            value * self.scale + self.zero
        } else {
            f32::from_bits(value as u32)
        }
    }
}

impl Keywords for F32Keywords {
    type T = f32;

    fn new(header: &Header<BinTable>, config: &TileCompressedImage) -> Self {
        let TileCompressedImage {
            z_dither_0,
            z_quantiz,
            ..
        } = config;

        let ctx = header.get_xtension();
        let z_scale_idx = ctx.find_field_by_ttype("ZSCALE");
        let z_zero_idx = ctx.find_field_by_ttype("ZZERO");

        let mut z_blank = None;
        let z_blank_idx = ctx
            // Check for a field named ZBLANK
            .find_field_by_ttype("ZBLANK")
            .or_else(|| {
                z_blank = header
                    .get_parsed("ZBLANK")
                    // TODO: we should probably propagate errors from here if ZBLANK/BLANK exist but are of a wrong type.
                    .ok();

                None
            });

        // If no ZBLANK colum has been found then check the header keywords (ZBLANK for float, BLANK for integer)
        let quantiz = if z_scale_idx.is_none() && z_zero_idx.is_none() {
            None
        } else {
            match z_quantiz {
                // Default is NO_DITHER when ZQUANTIZ keyword is not found
                Some(ZQuantiz::NoDither) | None => Some(Quantiz::NoDither),
                Some(ZQuantiz::SubtractiveDither1) => Some(Quantiz::SubtractiveDither1 { i1: 0 }),
                Some(ZQuantiz::SubtractiveDither2) => Some(Quantiz::SubtractiveDither2 { i1: 0 }),
            }
        };

        Self {
            scale: 1.0,
            zero: 0.0,
            z_scale_idx,
            z_zero_idx,
            z_blank_idx,
            z_blank,
            quantiz,
            z_dither_0: z_dither_0.unwrap_or(0),
        }
    }
}

impl Keywords for F64Keywords {
    type T = f64;

    fn new(header: &Header<BinTable>, config: &TileCompressedImage) -> Self {
        let TileCompressedImage {
            z_dither_0,
            z_quantiz,
            ..
        } = config;

        let ctx = header.get_xtension();
        let z_scale_idx = ctx.find_field_by_ttype("ZSCALE");
        let z_zero_idx = ctx.find_field_by_ttype("ZZERO");

        let mut z_blank = None;
        let z_blank_idx = ctx
            // Check for a field named ZBLANK
            .find_field_by_ttype("ZBLANK")
            .or_else(|| {
                z_blank = header
                    .get_parsed("ZBLANK")
                    // TODO: we should probably propagate errors from here if ZBLANK/BLANK exist but are of a wrong type.
                    .ok();

                None
            });

        // If no ZBLANK colum has been found then check the header keywords (ZBLANK for float, BLANK for integer)
        let quantiz = if z_scale_idx.is_none() && z_zero_idx.is_none() {
            None
        } else {
            match z_quantiz {
                // Default is NO_DITHER when ZQUANTIZ keyword is not found
                Some(ZQuantiz::NoDither) | None => Some(Quantiz::NoDither),
                Some(ZQuantiz::SubtractiveDither1) => Some(Quantiz::SubtractiveDither1 { i1: 0 }),
                Some(ZQuantiz::SubtractiveDither2) => Some(Quantiz::SubtractiveDither2 { i1: 0 }),
            }
        };

        Self {
            scale: 1.0,
            zero: 0.0,
            z_scale_idx,
            z_zero_idx,
            z_blank_idx,
            z_blank,
            quantiz,
            z_dither_0: z_dither_0.unwrap_or(0),
        }
    }
}

#[derive(Debug)]
pub struct U8Keywords {
    bzero: f32,
    bscale: f32,
    _blank: Option<u8>,
}

impl Keywords for U8Keywords {
    type T = u8;

    fn new(header: &Header<BinTable>, _: &TileCompressedImage) -> Self {
        let _blank = header.get_parsed::<Self::T>("BLANK").ok();

        let bzero = header.get_parsed::<f32>("BZERO").unwrap_or(0.0);
        let bscale = header.get_parsed::<f32>("BSCALE").unwrap_or(1.0);

        Self {
            _blank,
            bscale,
            bzero,
        }
    }
}

#[derive(Debug)]
pub struct I16Keywords {
    bzero: f32,
    bscale: f32,
    _blank: Option<i16>,
}

impl Keywords for I16Keywords {
    type T = i16;

    fn new(header: &Header<BinTable>, _: &TileCompressedImage) -> Self {
        let _blank = header.get_parsed::<Self::T>("BLANK").ok();

        let bzero = header.get_parsed::<f32>("BZERO").unwrap_or(0.0);
        let bscale = header.get_parsed::<f32>("BSCALE").unwrap_or(1.0);

        Self {
            _blank,
            bscale,
            bzero,
        }
    }
}

#[derive(Debug)]
pub struct I32Keywords {
    bzero: f32,
    bscale: f32,
    _blank: Option<i32>,
}

impl Keywords for I32Keywords {
    type T = i32;

    fn new(header: &Header<BinTable>, _: &TileCompressedImage) -> Self {
        let _blank = header.get_parsed::<Self::T>("BLANK").ok();

        let bzero = header.get_parsed::<f32>("BZERO").unwrap_or(0.0);
        let bscale = header.get_parsed::<f32>("BSCALE").unwrap_or(1.0);

        Self {
            _blank,
            bscale,
            bzero,
        }
    }
}

#[derive(Debug)]
enum Quantiz {
    NoDither,
    SubtractiveDither1 { i1: usize },
    SubtractiveDither2 { i1: usize },
}

#[cfg(test)]
mod tests {
    use crate::hdu::data::bintable::data::BinaryTableData;
    use image::DynamicImage;
    use std::io::{Cursor, Read};
    use test_case::test_case;

    use crate::{hdu::data::bintable::tile_compressed::pixels::Pixels, Fits, HDU};

    /*
    #[test]
    fn test_tile_size_from_row_idx() {
        use super::pixels::tile_size_from_row_idx;

        let ground_truth = [
            [300, 200, 150],
            [300, 200, 150],
            [300, 200, 150],
            [100, 200, 150],
            [300, 200, 150],
            [300, 200, 150],
            [300, 200, 150],
            [100, 200, 150],
            [300, 100, 150],
            [300, 100, 150],
            [300, 100, 150],
            [100, 100, 150],
            [300, 200, 150],
            [300, 200, 150],
            [300, 200, 150],
            [100, 200, 150],
            [300, 200, 150],
            [300, 200, 150],
            [300, 200, 150],
            [100, 200, 150],
            [300, 100, 150],
            [300, 100, 150],
            [300, 100, 150],
            [100, 100, 150],
            [300, 200, 50],
            [300, 200, 50],
            [300, 200, 50],
            [100, 200, 50],
            [300, 200, 50],
            [300, 200, 50],
            [300, 200, 50],
            [100, 200, 50],
            [300, 100, 50],
            [300, 100, 50],
            [300, 100, 50],
            [100, 100, 50],
        ];
        for (i, &ground_truth) in ground_truth.iter().enumerate() {
            let tile_s = tile_size_from_row_idx(&[300, 200, 150], &[1000, 500, 350], i);
            assert_eq!([tile_s[0], tile_s[1], tile_s[2]], ground_truth);
        }
    }*/

    use std::cmp::Ordering;
    use std::ops::Range;
    pub fn first_and_last_percent<T>(
        slice: &mut [T],
        mut first_percent: f32,
        mut last_percent: f32,
    ) -> Range<T>
    where
        T: PartialOrd + Copy,
    {
        if first_percent > last_percent {
            std::mem::swap(&mut first_percent, &mut last_percent);
        }

        let n = slice.len();
        let i1 = (first_percent * 0.01 * (n as f32)) as usize;
        let i2 = (last_percent * 0.01 * (n as f32)) as usize;

        let min_val = {
            let (_, min_val, _) = slice
                .select_nth_unstable_by(i1, |a, b| a.partial_cmp(b).unwrap_or(Ordering::Greater));
            *min_val
        };
        let max_val = {
            let (_, max_val, _) = slice
                .select_nth_unstable_by(i2, |a, b| a.partial_cmp(b).unwrap_or(Ordering::Greater));
            *max_val
        };

        min_val..max_val
    }

    #[test_case("samples/fits.gsfc.nasa.gov/m13real_rice.fits", 1000.0)]
    #[test_case("samples/fits.gsfc.nasa.gov/m13_rice.fits", 1000.0)]
    #[test_case("samples/fits.gsfc.nasa.gov/m13_gzip.fits", 1000.0)]
    fn test_fits_without_dithering(filename: &str, vmax: f32) {
        use std::fs::File;

        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let reader = Cursor::new(&buf[..]);

        let mut hdu_list = Fits::from_reader(reader);

        let mut idx = 0;
        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let img_bytes = match hdu_list.get_data(&hdu) {
                    BinaryTableData::TileCompressed(Pixels::U8(pixels)) => {
                        pixels.map(|(_, v)| v).collect::<Vec<_>>()
                    }
                    BinaryTableData::TileCompressed(Pixels::I16(pixels)) => pixels
                        .map(|(_, v)| (((v as f32) / vmax) * 255.0) as u8)
                        .collect::<Vec<_>>(),
                    BinaryTableData::TileCompressed(Pixels::I32(pixels)) => pixels
                        .map(|(_, v)| (((v as f32) / vmax) * 255.0) as u8)
                        .collect::<Vec<_>>(),
                    BinaryTableData::TileCompressed(Pixels::F32(pixels)) => pixels
                        .map(|(_, v)| ((v / vmax) * 255.0) as u8)
                        .collect::<Vec<_>>(),
                    _ => continue,
                };

                let width = hdu.get_header().get_parsed::<u32>("ZNAXIS1").unwrap();
                let height = hdu.get_header().get_parsed::<u32>("ZNAXIS2").unwrap();

                let imgbuf = DynamicImage::ImageLuma8(
                    image::ImageBuffer::from_raw(width, height, img_bytes).unwrap(),
                );
                imgbuf.save(format!("{filename}_ext{idx}.jpg")).unwrap();
            }
            idx += 1;
        }
    }

    #[test_case("samples/lsst/cutout.fits")]
    fn test_lsst_data(filename: &str) {
        use std::fs::File;

        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let reader = Cursor::new(&buf[..]);

        let mut hdu_list = Fits::from_reader(reader);

        let mut idx = 0;
        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let pixels = match hdu_list.get_data(&hdu) {
                    BinaryTableData::TileCompressed(Pixels::F32(pixels)) => {
                        let width = hdu.get_header().get_parsed::<u32>("ZNAXIS1").unwrap();
                        let height = hdu.get_header().get_parsed::<u32>("ZNAXIS2").unwrap();

                        let mut image_data = vec![0.0_f32; (width * height) as usize];

                        for (dst_idx, v) in pixels {
                            image_data[dst_idx] = v;
                        }

                        image_data
                    }
                    BinaryTableData::TileCompressed(Pixels::I32(pixels)) => {
                        let width = hdu.get_header().get_parsed::<u32>("ZNAXIS1").unwrap();
                        let height = hdu.get_header().get_parsed::<u32>("ZNAXIS2").unwrap();
                        let mut image_data = vec![0.0_f32; (width * height) as usize];

                        for (dst_idx, pixel) in pixels {
                            image_data[dst_idx] = pixel as f32;
                        }

                        image_data
                    }
                    _ => continue,
                };

                // compute the cuts
                let cuts = first_and_last_percent(&mut pixels.clone(), 0.01, 99.99);

                // apply the cuts
                let img_bytes = pixels
                    .into_iter()
                    .map(|v| {
                        (((v - cuts.start) / (cuts.end - cuts.start)) * 255.0).clamp(0.0, 255.0)
                            as u8
                    })
                    .collect::<Vec<_>>();

                let width = hdu.get_header().get_parsed::<u32>("ZNAXIS1").unwrap();
                let height = hdu.get_header().get_parsed::<u32>("ZNAXIS2").unwrap();

                let imgbuf = DynamicImage::ImageLuma8(
                    image::ImageBuffer::from_raw(width, height, img_bytes).unwrap(),
                );
                imgbuf.save(format!("{filename}_ext{idx}.jpg")).unwrap();
            }
            idx += 1;
        }
    }

    #[test_case("samples/fits.gsfc.nasa.gov/FITS RICE integer.fz")]
    fn test_fits_rice_integer(filename: &str) {
        use std::fs::File;

        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let reader = Cursor::new(&buf[..]);

        let mut hdu_list = Fits::from_reader(reader);

        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let width = hdu.get_header().get_parsed::<u32>("ZNAXIS1").unwrap();
                let height = hdu.get_header().get_parsed::<u32>("ZNAXIS2").unwrap();

                if let BinaryTableData::TileCompressed(Pixels::I32(pixels)) =
                    hdu_list.get_data(&hdu)
                {
                    let pixels = pixels
                        .map(|(_, v)| ((v as f32 / 100.0) * 255.0) as u8)
                        .collect::<Vec<_>>();

                    let imgbuf = DynamicImage::ImageLuma8(
                        image::ImageBuffer::from_raw(width, height, pixels).unwrap(),
                    );
                    imgbuf.save(format!("{filename}.jpg")).unwrap();
                }
            }
        }
    }

    #[test_case("samples/fits.gsfc.nasa.gov/FITS RICE_ONE.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/FITS RICE DITHER2 method.fz")]
    fn test_fits_f32_with_dithering(filename: &str) {
        use std::fs::File;

        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let reader = Cursor::new(&buf[..]);

        let mut hdu_list = Fits::from_reader(reader);

        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let width = hdu.get_header().get_parsed::<u32>("ZNAXIS1").unwrap();
                let height = hdu.get_header().get_parsed::<u32>("ZNAXIS2").unwrap();

                let mut buf = vec![0_u8; (width as usize) * (height as usize)];

                if let BinaryTableData::TileCompressed(Pixels::F32(pixels)) =
                    hdu_list.get_data(&hdu)
                {
                    for (i, pixel) in pixels {
                        buf[i] = (pixel * 255.0) as u8;
                    }
                }

                let imgbuf = DynamicImage::ImageLuma8(
                    image::ImageBuffer::from_raw(width, height, buf).unwrap(),
                );
                imgbuf.save(format!("{filename}.jpg")).unwrap();
            }
        }
    }
}
