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
    z_scale_idx: usize,
    /// Idx column storing z_zero values for each tile
    z_zero_idx: usize,
    /// Idx column storing z_blank values for each tile
    z_blank_idx: Option<usize>,

    /// Dither
    z_dither_0: i64,
    /// quantiz option
    z_quantiz: ZQuantiz,

    /// Current value of ZSCALE field (floating point case)
    scale: f32,
    /// Current value of ZZERO field (floating point case)
    zero: f32,
    /// Current value of ZBLANK (floating point case)
    /// Stores the integer value that evaluates to a floating point NAN
    z_blank: Option<i32>,
    /// Current value of ZBLANK field (float and integer case)
    /// Current quantiz state
    quantiz: Quantiz,
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

        match &mut self.quantiz {
            Quantiz::NoDither => (value as f32) * self.scale + self.zero,
            Quantiz::SubtractiveDither1 { i1 } => {
                let ri = RAND_VALUES[*i1];
                // increment i1 for the next pixel
                *i1 = (*i1 + 1) % N_RANDOM;

                ((value as f32) - ri + 0.5) * self.scale + self.zero
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

                    ((value as f32) - ri + 0.5) * self.scale + self.zero
                }
            }
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
        let z_scale_idx = ctx.find_field_by_ttype("ZSCALE").unwrap();
        let z_zero_idx = ctx.find_field_by_ttype("ZZERO").unwrap();

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

        Self {
            scale: 1.0,
            zero: 0.0,
            z_scale_idx,
            z_zero_idx,
            z_blank_idx,
            z_blank,
            quantiz: Quantiz::NoDither,
            z_quantiz: z_quantiz.clone().unwrap_or(ZQuantiz::NoDither),
            z_dither_0: z_dither_0.unwrap_or(0),
        }
    }
}

#[derive(Debug)]
pub struct U8Keywords {
    _blank: Option<u8>,
}

impl Keywords for U8Keywords {
    type T = u8;

    fn new(header: &Header<BinTable>, _: &TileCompressedImage) -> Self {
        let _blank = header.get_parsed::<Self::T>("BLANK").ok();

        Self { _blank }
    }
}

#[derive(Debug)]
pub struct I16Keywords {
    _blank: Option<i16>,
}

impl Keywords for I16Keywords {
    type T = i16;

    fn new(header: &Header<BinTable>, _: &TileCompressedImage) -> Self {
        let _blank = header.get_parsed::<Self::T>("BLANK").ok();

        Self { _blank }
    }
}

#[derive(Debug)]
pub struct I32Keywords {
    _blank: Option<i32>,
}

impl Keywords for I32Keywords {
    type T = i32;

    fn new(header: &Header<BinTable>, _: &TileCompressedImage) -> Self {
        let _blank = header.get_parsed::<Self::T>("BLANK").ok();

        Self { _blank }
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

    #[test]
    fn test_tile_size_from_row_idx() {
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
        use super::pixels::tile_size_from_row_idx;
        for (i, &ground_truth) in ground_truth.iter().enumerate() {
            let tile_s = tile_size_from_row_idx(&[300, 200, 150], &[1000, 500, 350], i);
            assert_eq!([tile_s[0], tile_s[1], tile_s[2]], ground_truth);
        }
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

        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let width = hdu.get_header().get_parsed::<u32>("ZNAXIS1").unwrap();
                let height = hdu.get_header().get_parsed::<u32>("ZNAXIS2").unwrap();

                let img_bytes = match hdu_list.get_data(&hdu) {
                    BinaryTableData::TileCompressed(Pixels::U8(pixels)) => {
                        pixels.collect::<Vec<_>>()
                    }
                    BinaryTableData::TileCompressed(Pixels::I16(pixels)) => pixels
                        .map(|v| (((v as f32) / vmax) * 255.0) as u8)
                        .collect::<Vec<_>>(),
                    BinaryTableData::TileCompressed(Pixels::I32(pixels)) => pixels
                        .map(|v| (((v as f32) / vmax) * 255.0) as u8)
                        .collect::<Vec<_>>(),
                    BinaryTableData::TileCompressed(Pixels::F32(pixels)) => pixels
                        .map(|v| ((v / vmax) * 255.0) as u8)
                        .collect::<Vec<_>>(),
                    _ => unreachable!(),
                };

                let imgbuf = DynamicImage::ImageLuma8(
                    image::ImageBuffer::from_raw(width, height, img_bytes).unwrap(),
                );
                imgbuf.save(format!("{filename}.jpg")).unwrap();
            }
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
                let bscale = hdu.get_header().get_parsed::<f32>("BSCALE").unwrap();
                let bzero = hdu.get_header().get_parsed::<f32>("BZERO").unwrap();

                if let BinaryTableData::TileCompressed(Pixels::I32(pixels)) =
                    hdu_list.get_data(&hdu)
                {
                    let pixels = pixels
                        .map(|v| ((((v as f32) * bscale + bzero) / 100.0) * 255.0) as u8)
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

                let tile_w = 100;
                let tile_h = 100;
                let num_tile_per_w = width / tile_w;

                if let BinaryTableData::TileCompressed(Pixels::F32(pixels)) =
                    hdu_list.get_data(&hdu)
                {
                    for (i, pixel) in pixels.map(|v| (v * 255.0) as u8).enumerate() {
                        let tile_idx = (i as u64) / ((tile_w * tile_h) as u64);
                        let x_tile_idx = tile_idx % (num_tile_per_w as u64);
                        let y_tile_idx = tile_idx / (num_tile_per_w as u64);

                        let pixel_inside_tile_idx = (i as u64) % ((tile_w * tile_h) as u64);
                        let x_pixel_inside_tile_idx = pixel_inside_tile_idx % (tile_w as u64);
                        let y_pixel_inside_tile_idx = pixel_inside_tile_idx / (tile_w as u64);

                        let x = x_tile_idx * (tile_w as u64) + x_pixel_inside_tile_idx;
                        let y = y_tile_idx * (tile_h as u64) + y_pixel_inside_tile_idx;

                        buf[(y * (width as u64) + x) as usize] = pixel;
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
