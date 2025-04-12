use flate2::read::GzDecoder;

use super::data::TableData;
use super::dithering::{N_RANDOM, RAND_VALUES};
use super::rice::RICEDecoder;
use crate::card::Value;
use crate::error::Error;
use crate::hdu::header::extension::bintable::{BinTable, TileCompressedImage, ZCmpType, ZQuantiz};
use crate::hdu::header::{Bitpix, Header};

use super::row::TableRowData;

#[derive(Debug)]
pub struct TileCompressedData<R> {
    /// An iterator over the row of a binary table
    pub(crate) row_it: TableRowData<R>,
    /// A buffer for storing uncompressed data from GZIP1, GZIP2 or RICE
    buf: Vec<u8>,

    // Mandatory data compressed column
    data_compressed_idx: usize,
    // Optional floating point columns
    z_scale_idx: Option<usize>,
    z_zero_idx: Option<usize>,
    // Optional column or keyword
    z_blank: Option<ZBLANK>,
    /// The type of compression algorithm
    z_cmp_type: ZCmpType,
    /// Bitpix of the uncompressed values
    z_bitpix: Bitpix,
    /// Tile size
    z_tile: Box<[usize]>,
    /// Total image size
    z_naxis: Box<[usize]>,
    /// optional z_dither0
    z_dither_0: Option<i64>,
    /// optional z_quantiz,
    z_quantiz: Option<ZQuantiz>,

    /// Current tile pointer
    tile: TileCompressed,
}

/// Proper data relative to the tile currently being parsed
#[derive(Debug)]
struct TileCompressed {
    /// Current value of ZSCALE field (floating point case)
    z_scale: f32,
    /// Current value of ZZERO field (floating point case)
    z_zero: f32,
    /// Current value of ZBLANK field (float and integer case)
    z_blank: Option<f32>,
    /// Total number of pixels in the tile
    n_pixels: u64,
    /// Number of pixels remaining to return
    remaining_pixels: u64,
    /// Quantiz parameters
    quantiz: Quantiz,
}

impl TileCompressed {
    /// Unquantize the integer decoded value to the real floating point value
    fn unquantize(&mut self, value: i32) -> f32 {
        match &mut self.quantiz {
            Quantiz::NoDither => (value as f32) * self.z_scale + self.z_zero,
            Quantiz::SubtractiveDither1 { i1 } => {
                let ri = RAND_VALUES[*i1];
                // increment i1 for the next pixel
                *i1 = (*i1 + 1) % N_RANDOM;

                ((value as f32) - ri + 0.5) * self.z_scale + self.z_zero
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

                    ((value as f32) - ri + 0.5) * self.z_scale + self.z_zero
                }
            }
        }
    }
}

#[derive(Debug)]
enum ZBLANK {
    ColumnIdx(usize),
    Value(f64),
}

#[derive(Debug)]
enum Quantiz {
    NoDither,
    SubtractiveDither1 { i1: usize },
    SubtractiveDither2 { i1: usize },
}

use std::fmt::Debug;
use std::io::{Read, Seek, SeekFrom};

/// Compute the size of the tile from its row position inside the compressed data column
/// FIXME: optimize this computation by using memoization
fn tile_size_from_row_idx(z_tile: &[usize], z_naxis: &[usize], n: usize) -> Box<[usize]> {
    let d = z_tile.len();

    if d == 0 {
        // There must be at least one dimension
        unreachable!();
    } else {
        let mut u = vec![0_usize; z_tile.len()];

        let s = z_naxis
            .iter()
            .zip(z_tile.iter())
            .map(|(naxisi, tilei)| naxisi.div_ceil(*tilei))
            .collect::<Vec<_>>();

        // Compute the position inside the first dimension
        u[0] = n % s[0];

        for i in 1..=(d - 1) {
            u[i] = n
                - u[0]
                - (1..=(i - 1))
                    .map(|k| {
                        let prod_sk = s.iter().take(k).product::<usize>();
                        u[k] * prod_sk
                    })
                    .sum::<usize>();

            let prod_si = s.iter().take(i).product::<usize>();

            u[i] = (u[i] / prod_si) % s[i];
        }

        u.iter()
            .zip(z_naxis.iter().zip(z_tile.iter()))
            .map(|(&u_i, (&naxis, &tilez))| tilez.min(naxis - u_i * tilez))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

impl<R> TileCompressedData<R> {
    fn get_reader(&mut self) -> &mut R {
        self.row_it.get_reader()
    }
}

impl<R> TileCompressedData<R> {
    pub(crate) fn new(
        header: &Header<BinTable>,
        mut data: TableData<R>,
        config: &TileCompressedImage,
    ) -> Self {
        // This buffer is only used if tile compressed image in the gzip compression is to be found
        let mut buf = vec![];

        let TileCompressedImage {
            z_tilen,
            z_naxisn,
            z_bitpix,
            z_cmp_type,
            data_compressed_idx,
            z_quantiz,
            z_dither_0,
            ..
        } = config;
        // Disable reading the heap, we will handle that manually after reading the column
        data.read_the_heap(false);
        let row_it = data.row_iter();

        let ctx = header.get_xtension();

        let z_scale_idx = ctx.find_field_by_ttype("ZSCALE");
        let z_zero_idx = ctx.find_field_by_ttype("ZZERO");

        // Allocation of a buffer at init of the iterator that is the size of the biggest tiles we can found
        // on the tile compressed image. This is simply given by the ZTILEi keyword.
        // Some tiles found on the border of the image can be smaller
        let n_elems_max = z_tilen.iter().fold(1, |mut tile_size, z_tilei| {
            tile_size *= *z_tilei;
            tile_size
        });

        // FIXME
        // A little precision. The gzdecoder from flate2 seems to unzip data in a stream of u32 i.e. even if the type of data
        // to uncompress is byte or short, the result will be cast in a u32. I thus allocate for gzip compression, a buffer of
        // n_elems_max * size_in_bytes of u32.
        // It seems to be the same for RICE, so the latter rice decompression code is called on i32
        let num_bytes_max_tile = n_elems_max * std::mem::size_of::<u32>();

        buf.resize(num_bytes_max_tile, 0);

        let z_blank = header
            .get_xtension()
            // Check for a field named ZBLANK
            .find_field_by_ttype("ZBLANK")
            // If no ZBLANK colum has been found then check the header keywords (ZBLANK for float, BLANK for integer)
            .map_or_else(
                || {
                    // integers
                    if (*z_bitpix as i32) < 0 {
                        if let Some(Value::Float { value, .. }) = header.get("ZBLANK") {
                            Some(ZBLANK::Value(*value))
                        } else {
                            None
                        }
                    } else if let Some(Value::Integer { value, .. }) = header.get("BLANK") {
                        Some(ZBLANK::Value(*value as f64))
                    } else {
                        None
                    }
                },
                |field_idx| Some(ZBLANK::ColumnIdx(field_idx)),
            );
        let tile = TileCompressed {
            z_scale: 1.0,
            z_zero: 0.0,
            z_blank: None,
            n_pixels: 0,
            remaining_pixels: 0,
            quantiz: Quantiz::NoDither,
        };

        Self {
            buf,
            row_it,
            tile,
            data_compressed_idx: *data_compressed_idx,
            z_scale_idx,
            z_zero_idx,
            z_blank,
            z_naxis: z_naxisn.clone(),
            z_tile: z_tilen.clone(),
            z_cmp_type: *z_cmp_type,
            z_bitpix: *z_bitpix,
            z_dither_0: *z_dither_0,
            z_quantiz: z_quantiz.clone(),
        }
    }

    /// Get an iterator over the binary table without interpreting its content as
    /// a compressed tile.
    ///
    /// This can be useful if you want to have access to the raw data because [TableData] has a method
    /// to get its raw_bytes
    pub fn table_data(self) -> TableData<R> {
        self.row_it.table_data()
    }
}

use super::{ColumnId, DataValue};
impl<R> Iterator for TileCompressedData<R>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = DataValue;

    fn next(&mut self) -> Option<Self::Item> {
        // We first retrieve the whole row from the row data iterator
        if self.tile.remaining_pixels == 0 {
            let row_data = self.row_it.next()?;

            let (_, byte_offset) = match row_data[self.data_compressed_idx] {
                DataValue::VariableLengthArray32 {
                    num_elems,
                    offset_byte,
                } => (num_elems as u64, offset_byte as u64),
                DataValue::VariableLengthArray64 {
                    num_elems,
                    offset_byte,
                } => (num_elems, offset_byte),
                _ => unreachable!(),
            };

            let ctx = self.row_it.get_ctx();
            let row_idx = self.row_it.get_row_idx();

            // Update the tile compressed currently decompressed
            let num_pixels = tile_size_from_row_idx(&self.z_tile[..], &self.z_naxis[..], row_idx)
                .iter()
                .fold(1, |mut n, &tile| {
                    n *= tile;
                    n
                }) as u64;
            self.tile.n_pixels = num_pixels;
            self.tile.remaining_pixels = num_pixels;
            self.tile.quantiz = match (&self.z_quantiz, self.z_dither_0) {
                (Some(ZQuantiz::NoDither), _) => Quantiz::NoDither,
                (Some(ZQuantiz::SubtractiveDither1), Some(zdither0)) => {
                    let i0 = (row_idx - 1 + (zdither0 as usize)) % 10000;
                    let i1 = (RAND_VALUES[i0] * 500.0).floor() as usize;
                    Quantiz::SubtractiveDither1 { i1 }
                }
                (Some(ZQuantiz::SubtractiveDither2), Some(zdither0)) => {
                    let i0 = (row_idx - 1 + (zdither0 as usize)) % 10000;
                    let i1 = (RAND_VALUES[i0] * 500.0).floor() as usize;
                    Quantiz::SubtractiveDither2 { i1 }
                }
                _ => Quantiz::NoDither,
            };

            self.tile.z_scale = if let Some(idx) = self.z_scale_idx {
                match row_data[idx] {
                    DataValue::Float { value, .. } => value,
                    DataValue::Double { value, .. } => value as f32,
                    _ => unreachable!(),
                }
            } else {
                1.0
            };

            self.tile.z_zero = if let Some(idx) = self.z_zero_idx {
                match row_data[idx] {
                    DataValue::Float { value, .. } => value,
                    DataValue::Double { value, .. } => value as f32,
                    _ => unreachable!(),
                }
            } else {
                0.0
            };

            self.tile.z_blank = match self.z_blank {
                Some(ZBLANK::ColumnIdx(idx)) => match row_data[idx] {
                    DataValue::Float { value, .. } => Some(value),
                    DataValue::Double { value, .. } => Some(value as f32),
                    _ => unreachable!(),
                },
                Some(ZBLANK::Value(value)) => Some(value as f32),
                _ => None,
            };

            // We jump to the heap at the position of the tile
            // Then we decomp the tile and store it into out internal buf
            // Finally we go back to the main data table location before jumping to the heap
            let main_data_table_offset = row_idx * (ctx.naxis1 as usize);
            let off =
                // go back to the beginning of the main table data block
                - (main_data_table_offset as i64)
                // from the beginning of the main table go to the beginning of the heap
                + ctx.theap as i64
                // from the beginning of the heap go to the start of the array
                + byte_offset as i64;
            self.jump_to_location(
                |s| {
                    let TileCompressedData { buf, row_it, .. } = s;

                    let reader = row_it.get_reader();

                    match s.z_cmp_type {
                        // For GZIP2, the byte shuffling is done in the next method
                        ZCmpType::Gzip1 | ZCmpType::Gzip2 => {
                            let mut gz = GzDecoder::new(reader);
                            gz.read_exact(&mut buf[..])?;
                        }
                        // FIXME support bytepix
                        ZCmpType::Rice { blocksize, .. } => {
                            let mut rice = RICEDecoder::<_, i32>::new(
                                reader,
                                blocksize as i32,
                                num_pixels as i32,
                            );
                            rice.read_exact(&mut buf[..])?;
                        }
                        // Other compression not supported, when parsing the bintable extension keywords
                        // we ensured that z_image is `None` for other compressions than GZIP or RICE
                        _ => unreachable!(),
                    }

                    Ok(())
                },
                SeekFrom::Current(off),
            )
            .ok()?;
        }

        // There is remaining pixels inside our buffer, we simply return the current one
        let idx = (self.tile.n_pixels - self.tile.remaining_pixels) as usize;
        let column = ColumnId::Index(self.data_compressed_idx);

        let value = match (self.z_cmp_type, self.z_bitpix) {
            // GZIP compression
            // Unsigned byte
            (ZCmpType::Gzip1, Bitpix::U8) | (ZCmpType::Gzip2, Bitpix::U8) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let value = self.buf[off + 3];
                DataValue::UnsignedByte { value, column, idx }
            }
            // 16-bit integer
            (ZCmpType::Gzip1, Bitpix::I16) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let off = 4 * idx;
                let value = (self.buf[off + 3] as i16) | ((self.buf[off + 2] as i16) << 8);
                DataValue::Short { value, column, idx }
            }
            // 32-bit integer
            (ZCmpType::Gzip1, Bitpix::I32) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = i32::from_be_bytes([
                    self.buf[off],
                    self.buf[off + 1],
                    self.buf[off + 2],
                    self.buf[off + 3],
                ]);
                DataValue::Integer { value, column, idx }
            }
            // 32-bit floating point
            (ZCmpType::Gzip1, Bitpix::F32) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = i32::from_be_bytes([
                    self.buf[off],
                    self.buf[off + 1],
                    self.buf[off + 2],
                    self.buf[off + 3],
                ]);
                let value = self.tile.unquantize(value);

                DataValue::Float { value, column, idx }
            }
            // GZIP2 (when uncompressed, the bytes are all in most signicant byte order)
            // FIXME: not tested
            // 16-bit integer
            (ZCmpType::Gzip2, Bitpix::I16) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let num_bytes = self.buf.len();
                let step_msb = num_bytes / 4;
                let value = (self.buf[3 * step_msb + idx] as i16)
                    | ((self.buf[2 * step_msb + idx] as i16) << 8);
                DataValue::Short { value, column, idx }
            }
            // 32-bit integer
            // FIXME: not tested
            (ZCmpType::Gzip2, Bitpix::I32) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let num_bytes = self.buf.len();
                let step_msb = num_bytes / 4;
                let value = ((self.buf[idx] as i32) << 24)
                    | ((self.buf[idx + step_msb] as i32) << 16)
                    | ((self.buf[idx + 2 * step_msb] as i32) << 8)
                    | (self.buf[idx + 3 * step_msb] as i32);

                DataValue::Integer { value, column, idx }
            }
            // 32-bit floating point
            (ZCmpType::Gzip2, Bitpix::F32) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let num_bytes = self.buf.len();
                let step_msb = num_bytes / 4;
                let value = ((self.buf[idx] as i32) << 24)
                    | ((self.buf[idx + step_msb] as i32) << 16)
                    | ((self.buf[idx + 2 * step_msb] as i32) << 8)
                    | (self.buf[idx + 3 * step_msb] as i32);

                let value = self.tile.unquantize(value);

                DataValue::Float { value, column, idx }
            }
            // RICE compression
            // Unsigned byte
            (ZCmpType::Rice { .. }, Bitpix::U8) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = self.buf[off];
                DataValue::UnsignedByte { value, column, idx }
            }
            // 16-bit integer
            (ZCmpType::Rice { .. }, Bitpix::I16) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = (self.buf[off] as i16) | ((self.buf[off + 1] as i16) << 8);
                DataValue::Short { value, column, idx }
            }
            // 32-bit integer
            (ZCmpType::Rice { .. }, Bitpix::I32) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = i32::from_ne_bytes([
                    self.buf[off],
                    self.buf[off + 1],
                    self.buf[off + 2],
                    self.buf[off + 3],
                ]);
                DataValue::Integer { value, column, idx }
            }
            // 32-bit floating point
            (ZCmpType::Rice { .. }, Bitpix::F32) => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = i32::from_ne_bytes([
                    self.buf[off],
                    self.buf[off + 1],
                    self.buf[off + 2],
                    self.buf[off + 3],
                ]);

                let value = self.tile.unquantize(value);

                DataValue::Float { value, column, idx }
            }
            // Not supported compression/bitpix results in parsing the binary table as normal and thus this part is not reachable
            _ => unreachable!(),
        };
        self.tile.remaining_pixels -= 1;

        Some(value)
    }
}

impl<R> TileCompressedData<R>
where
    R: Seek,
{
    /// Jump to a specific location of the reader, perform an operation and jumps back to the original position
    pub(crate) fn jump_to_location<F>(&mut self, f: F, pos: SeekFrom) -> Result<(), Error>
    where
        F: FnOnce(&mut Self) -> Result<(), Error>,
    {
        let old_pos = SeekFrom::Start(self.get_reader().stream_position()?);

        self.get_reader().seek(pos)?;
        f(self)?;
        let _ = self.get_reader().seek(old_pos)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use image::DynamicImage;
    use test_case::test_case;

    use crate::{Fits, HDU};

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
        use super::tile_size_from_row_idx;
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

        use crate::hdu::data::bintable::DataValue;

        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let reader = Cursor::new(&buf[..]);

        let mut hdu_list = Fits::from_reader(reader);

        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let width = hdu
                    .get_header()
                    .get_parsed::<i64>("ZNAXIS1")
                    .unwrap()
                    .unwrap() as u32;
                let height = hdu
                    .get_header()
                    .get_parsed::<i64>("ZNAXIS2")
                    .unwrap()
                    .unwrap() as u32;
                let pixels = hdu_list
                    .get_data(&hdu)
                    .map(|value| match value {
                        DataValue::Short { value, .. } => value as f32,
                        DataValue::Integer { value, .. } => value as f32,
                        DataValue::Float { value, .. } => value,
                        _ => unimplemented!(),
                    })
                    .map(|v| ((v / vmax) * 255.0) as u8)
                    .collect::<Vec<_>>();

                let imgbuf = DynamicImage::ImageLuma8(
                    image::ImageBuffer::from_raw(width, height, pixels).unwrap(),
                );
                imgbuf.save(format!("{}.jpg", filename)).unwrap();
            }
        }
    }

    #[test_case("samples/fits.gsfc.nasa.gov/FITS RICE integer.fz")]
    fn test_fits_rice_integer(filename: &str) {
        use std::fs::File;

        use crate::hdu::data::bintable::DataValue;

        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let reader = Cursor::new(&buf[..]);

        let mut hdu_list = Fits::from_reader(reader);

        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let width = hdu
                    .get_header()
                    .get_parsed::<i64>("ZNAXIS1")
                    .unwrap()
                    .unwrap() as u32;
                let height = hdu
                    .get_header()
                    .get_parsed::<i64>("ZNAXIS2")
                    .unwrap()
                    .unwrap() as u32;
                let bscale = hdu
                    .get_header()
                    .get_parsed::<f64>("BSCALE")
                    .unwrap()
                    .unwrap() as f32;
                let bzero = hdu
                    .get_header()
                    .get_parsed::<f64>("BZERO")
                    .unwrap()
                    .unwrap() as f32;

                let pixels = hdu_list
                    .get_data(&hdu)
                    .map(|value| match value {
                        DataValue::Short { value, .. } => value as f32,
                        DataValue::Integer { value, .. } => value as f32,
                        DataValue::Float { value, .. } => value,
                        _ => unimplemented!(),
                    })
                    .map(|v| (((v * bscale + bzero) / 100.0) * 255.0) as u8)
                    .collect::<Vec<_>>();

                let imgbuf = DynamicImage::ImageLuma8(
                    image::ImageBuffer::from_raw(width, height, pixels).unwrap(),
                );
                imgbuf.save(format!("{}.jpg", filename)).unwrap();
            }
        }
    }

    #[test_case("samples/fits.gsfc.nasa.gov/FITS RICE_ONE.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/FITS RICE DITHER2 method.fz")]
    fn test_fits_f32_with_dithering(filename: &str) {
        use std::fs::File;

        use crate::hdu::data::bintable::DataValue;

        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let reader = Cursor::new(&buf[..]);

        let mut hdu_list = Fits::from_reader(reader);

        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let width = hdu
                    .get_header()
                    .get_parsed::<i64>("ZNAXIS1")
                    .unwrap()
                    .unwrap() as u32;
                let height = hdu
                    .get_header()
                    .get_parsed::<i64>("ZNAXIS2")
                    .unwrap()
                    .unwrap() as u32;

                let mut buf = vec![0_u8; (width as usize) * (height as usize)];

                let tile_w = 100;
                let tile_h = 100;
                let num_tile_per_w = width / tile_w;

                for (i, pixel) in hdu_list
                    .get_data(&hdu)
                    .map(|value| match value {
                        DataValue::Short { value, .. } => value as f32,
                        DataValue::Integer { value, .. } => value as f32,
                        DataValue::Float { value, .. } => value,
                        _ => unimplemented!(),
                    })
                    .map(|v| (v * 255.0) as u8)
                    .enumerate()
                {
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

                let imgbuf = DynamicImage::ImageLuma8(
                    image::ImageBuffer::from_raw(width, height, buf).unwrap(),
                );
                imgbuf.save(format!("{}.jpg", filename)).unwrap();
            }
        }
    }
}
