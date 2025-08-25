use flate2::read::GzDecoder;

use super::super::DataValue;
use super::dithering::RAND_VALUES;
use super::rice::RICEDecoder;
use super::{F32Keywords, I16Keywords, I32Keywords, Keywords, Quantiz, TileDesc, U8Keywords};
use crate::error::Error;
use crate::hdu::header::extension::bintable::{BinTable, TileCompressedImage, ZCmpType, ZQuantiz};
use crate::hdu::header::{Bitpix, Header};
use crate::{TableData, TableRowData};
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub enum Pixels<R> {
    U8(It<R, U8Keywords>),
    I16(It<R, I16Keywords>),
    I32(It<R, I32Keywords>),
    F32(It<R, F32Keywords>),
}

impl<R> Pixels<R> {
    pub fn new(
        data: TableData<R>,
        header: &Header<BinTable>,
        config: &TileCompressedImage,
    ) -> Self {
        match config.z_bitpix {
            Bitpix::U8 => Self::U8(It::new(header, data, config)),
            Bitpix::I16 => Self::I16(It::new(header, data, config)),
            Bitpix::I32 => Self::I32(It::new(header, data, config)),
            Bitpix::F32 => Self::F32(It::new(header, data, config)),
            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct It<R, K>
where
    K: Keywords,
{
    /// An iterator over the row of a binary table
    pub row_it: TableRowData<R>,
    /// A buffer for storing uncompressed data from GZIP1, GZIP2 or RICE
    buf: Vec<u8>,

    /// Current tile pointer
    desc: TileDesc<K>,

    z_tile: Box<[usize]>,
    z_naxis: Box<[usize]>,
    z_cmp_type: ZCmpType,
    data_compressed_idx: usize,
}

impl<R, K> It<R, K>
where
    K: Keywords,
{
    fn get_reader(&mut self) -> &mut R {
        self.row_it.get_reader()
    }
}

impl<R, K> It<R, K>
where
    K: Keywords,
{
    pub(crate) fn new(
        header: &Header<BinTable>,
        mut data: TableData<R>,
        config: &TileCompressedImage,
    ) -> Self {
        // This buffer is only used if tile compressed image in the gzip compression is to be found
        let TileCompressedImage {
            z_tilen,
            z_naxisn,
            z_cmp_type,
            data_compressed_idx,
            ..
        } = config;
        // Allocation of a buffer at init of the iterator that is the size of the biggest tiles we can found
        // on the tile compressed image. This is simply given by the ZTILEi keyword.
        // Some tiles found on the border of the image can be smaller
        let n_elems_max = z_tilen.iter().product::<usize>();

        // FIXME
        // A little precision. The gzdecoder from flate2 seems to unzip data in a stream of u32 i.e. even if the type of data
        // to uncompress is byte or short, the result will be cast in a u32. I thus allocate for gzip compression, a buffer of
        // n_elems_max * size_in_bytes of u32.
        // It seems to be the same for RICE, so the latter rice decompression code is called on i32
        let num_bytes_max_tile = n_elems_max * std::mem::size_of::<u32>();

        let buf = vec![0_u8; num_bytes_max_tile];

        let desc = TileDesc::new(header, config);

        // do not read the heap, we will manage our way decompressing the tiles
        data.read_the_heap(false);
        Self {
            buf,
            row_it: data.row_iter(),

            desc,

            data_compressed_idx: *data_compressed_idx,
            z_naxis: z_naxisn.clone(),
            z_tile: z_tilen.clone(),
            z_cmp_type: *z_cmp_type,
        }
    }
}

/// Compute the size of the tile from its row position inside the compressed data column
/// FIXME: optimize this computation by using memoization
pub(crate) fn tile_size_from_row_idx(
    z_tile: &[usize],
    z_naxis: &[usize],
    n: usize,
) -> Box<[usize]> {
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

        for i in 1..d {
            u[i] = n
                - u[0]
                - (1..i)
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
            .collect()
    }
}

use std::fmt::Debug;
impl<R> Iterator for It<R, U8Keywords>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        // We first retrieve the whole row from the row data iterator
        if self.desc.remaining_pixels == 0 {
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
                .product::<usize>() as u64;
            self.desc.n_pixels = num_pixels;
            self.desc.remaining_pixels = num_pixels;

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
                    let It { buf, row_it, .. } = s;

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
        let idx = (self.desc.n_pixels - self.desc.remaining_pixels) as usize;

        let value = match self.z_cmp_type {
            ZCmpType::Gzip1 | ZCmpType::Gzip2 => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                self.buf[off + 3]
            }
            // GZIP compression
            // Unsigned byte
            /*
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
            }*/
            // 32-bit floating point
            /*// GZIP2 (when uncompressed, the bytes are all in most signicant byte order)
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
            }*/
            ZCmpType::Rice { .. } => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = self.buf[off];
                value
            }
            /*// RICE compression
            // Unsigned byte

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
            }*/
            // Not supported compression/bitpix results in parsing the binary table as normal and thus this part is not reachable
            _ => unreachable!(),
        };
        self.desc.remaining_pixels -= 1;

        Some(value)
    }
}

impl<R> Iterator for It<R, I16Keywords>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        // We first retrieve the whole row from the row data iterator
        if self.desc.remaining_pixels == 0 {
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
                .product::<usize>() as u64;
            self.desc.n_pixels = num_pixels;
            self.desc.remaining_pixels = num_pixels;
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
                    let It { buf, row_it, .. } = s;

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
        let idx = (self.desc.n_pixels - self.desc.remaining_pixels) as usize;

        let value = match self.z_cmp_type {
            ZCmpType::Gzip1 => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let off = 4 * idx;
                (self.buf[off + 3] as i16) | ((self.buf[off + 2] as i16) << 8)
            }
            ZCmpType::Gzip2 => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let num_bytes = self.buf.len();
                let step_msb = num_bytes / 4;
                (self.buf[3 * step_msb + idx] as i16) | ((self.buf[2 * step_msb + idx] as i16) << 8)
            }
            // GZIP compression
            // Unsigned byte
            /*
            // 16-bit integer

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
            }*/
            // 32-bit floating point
            /*// GZIP2 (when uncompressed, the bytes are all in most signicant byte order)
            // FIXME: not tested
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
            }*/
            ZCmpType::Rice { .. } => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                (self.buf[off] as i16) | ((self.buf[off + 1] as i16) << 8)
            }
            /*// RICE compression
            // Unsigned byte

            // 16-bit integer

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
            }*/
            // Not supported compression/bitpix results in parsing the binary table as normal and thus this part is not reachable
            _ => unreachable!(),
        };
        self.desc.remaining_pixels -= 1;

        Some(value)
    }
}

impl<R> Iterator for It<R, I32Keywords>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        // We first retrieve the whole row from the row data iterator
        if self.desc.remaining_pixels == 0 {
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
                .product::<usize>() as u64;
            self.desc.n_pixels = num_pixels;
            self.desc.remaining_pixels = num_pixels;
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
                    let It { buf, row_it, .. } = s;

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
        let idx = (self.desc.n_pixels - self.desc.remaining_pixels) as usize;

        let value = match self.z_cmp_type {
            ZCmpType::Gzip1 => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                i32::from_be_bytes([
                    self.buf[off],
                    self.buf[off + 1],
                    self.buf[off + 2],
                    self.buf[off + 3],
                ])
            }
            ZCmpType::Gzip2 => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let num_bytes = self.buf.len();
                let step_msb = num_bytes / 4;
                ((self.buf[idx] as i32) << 24)
                    | ((self.buf[idx + step_msb] as i32) << 16)
                    | ((self.buf[idx + 2 * step_msb] as i32) << 8)
                    | (self.buf[idx + 3 * step_msb] as i32)
            }
            ZCmpType::Rice { .. } => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                i32::from_ne_bytes([
                    self.buf[off],
                    self.buf[off + 1],
                    self.buf[off + 2],
                    self.buf[off + 3],
                ])
            }
            // Not supported compression/bitpix results in parsing the binary table as normal and thus this part is not reachable
            _ => unreachable!(),
        };
        self.desc.remaining_pixels -= 1;

        Some(value)
    }
}

impl<R> Iterator for It<R, F32Keywords>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        // We first retrieve the whole row from the row data iterator
        if self.desc.remaining_pixels == 0 {
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
                .product::<usize>() as u64;
            self.desc.n_pixels = num_pixels;
            self.desc.remaining_pixels = num_pixels;

            let TileDesc {
                keywords:
                    F32Keywords {
                        z_scale_idx,
                        z_zero_idx,
                        scale,
                        zero,
                        z_dither_0,
                        z_quantiz,
                        quantiz,
                        ..
                    },
                ..
            } = &mut self.desc;

            *quantiz = match z_quantiz {
                ZQuantiz::SubtractiveDither1 => {
                    let i0 = (row_idx - 1 + ((*z_dither_0) as usize)) % 10000;
                    let i1 = (RAND_VALUES[i0] * 500.0).floor() as usize;
                    Quantiz::SubtractiveDither1 { i1 }
                }
                ZQuantiz::SubtractiveDither2 => {
                    let i0 = (row_idx - 1 + (*z_dither_0 as usize)) % 10000;
                    let i1 = (RAND_VALUES[i0] * 500.0).floor() as usize;
                    Quantiz::SubtractiveDither2 { i1 }
                }
                _ => Quantiz::NoDither,
            };

            *scale = match row_data[*z_scale_idx] {
                DataValue::Float { value, .. } => value,
                DataValue::Double { value, .. } => value as f32,
                _ => unreachable!(),
            };

            *zero = match row_data[*z_zero_idx] {
                DataValue::Float { value, .. } => value,
                DataValue::Double { value, .. } => value as f32,
                _ => unreachable!(),
            };

            /*
            *z_blank = match self.z_blank {
                Some(ZBLANK::ColumnIdx(idx)) => match row_data[idx] {
                    DataValue::Float { value, .. } => Some(value),
                    DataValue::Double { value, .. } => Some(value as f32),
                    _ => unreachable!(),
                },
                Some(ZBLANK::Value(value)) => Some(value as f32),
                _ => None,
            };
            */
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
                    let It { buf, row_it, .. } = s;

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
        let idx = (self.desc.n_pixels - self.desc.remaining_pixels) as usize;

        let value = match self.z_cmp_type {
            // GZIP compression
            // Unsigned byte
            /*(ZCmpType::Gzip1, Bitpix::U8) | (ZCmpType::Gzip2, Bitpix::U8) => {
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
            }*/
            // 32-bit floating point
            ZCmpType::Gzip1 => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = i32::from_be_bytes([
                    self.buf[off],
                    self.buf[off + 1],
                    self.buf[off + 2],
                    self.buf[off + 3],
                ]);
                self.desc.keywords.unquantize(value)
            }
            /*// GZIP2 (when uncompressed, the bytes are all in most signicant byte order)
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
            }*/
            // 32-bit floating point
            ZCmpType::Gzip2 => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let num_bytes = self.buf.len();
                let step_msb = num_bytes / 4;
                let value = ((self.buf[idx] as i32) << 24)
                    | ((self.buf[idx + step_msb] as i32) << 16)
                    | ((self.buf[idx + 2 * step_msb] as i32) << 8)
                    | (self.buf[idx + 3 * step_msb] as i32);

                self.desc.keywords.unquantize(value)
            }
            /*// RICE compression
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
            }*/
            // 32-bit floating point
            ZCmpType::Rice { .. } => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                let value = i32::from_ne_bytes([
                    self.buf[off],
                    self.buf[off + 1],
                    self.buf[off + 2],
                    self.buf[off + 3],
                ]);

                self.desc.keywords.unquantize(value)
            }
            // Not supported compression/bitpix results in parsing the binary table as normal and thus this part is not reachable
            _ => unreachable!(),
        };
        self.desc.remaining_pixels -= 1;

        Some(value)
    }
}

impl<R, K> It<R, K>
where
    R: Seek,
    K: Keywords,
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
