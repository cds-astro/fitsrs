use flate2::read::GzDecoder;

use super::super::DataValue;
use super::dithering::RAND_VALUES;
use super::rice::RICEDecoder;
use super::{
    F32Keywords, F64Keywords, I16Keywords, I32Keywords, Keywords, Quantiz, TileDesc, U8Keywords,
};
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
    F64(It<R, F64Keywords>),
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
            Bitpix::F64 => Self::F64(It::new(header, data, config)),
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
    /// Return the actual number of pixels in the tile represented by a BINTABLE row.
    /// Edge tiles can be smaller than ZTILEn. get_row_idx() is FITS-style (1-based).
    fn tile_pixel_count(&self, row_idx: usize) -> usize {
        let n = row_idx.saturating_sub(1);
        let d = self.z_tile.len();
        let tiles_per_axis = self
            .z_naxis
            .iter()
            .zip(self.z_tile.iter())
            .map(|(&naxis, &tile)| naxis.div_ceil(tile))
            .collect::<Vec<_>>();

        let mut coord = vec![0usize; d];
        coord[0] = n % tiles_per_axis[0];
        for i in 1..d {
            let stride = tiles_per_axis.iter().take(i).product::<usize>();
            coord[i] = (n / stride) % tiles_per_axis[i];
        }

        coord
            .iter()
            .zip(self.z_naxis.iter().zip(self.z_tile.iter()))
            .map(|(&c, (&naxis, &tile))| tile.min(naxis.saturating_sub(c * tile)))
            .product()
    }

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
            z_cmp_type,
            data_compressed_idx,
            z_naxisn,
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
            z_tile: z_tilen.clone(),
            z_naxis: z_naxisn.iter().map(|znaxis| *znaxis as usize).collect::<Vec<_>>().into_boxed_slice(),
            z_cmp_type: *z_cmp_type,
        }
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
            let num_pixels = self.tile_pixel_count(row_idx) as u64;
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
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
                            let mut gz = GzDecoder::new(reader);
                            gz.read_exact(&mut buf[..])?;
                        }
                        // FIXME support bytepix
                        ZCmpType::Rice { blocksize, .. } => {
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
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
            ZCmpType::Rice { .. } => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                self.buf[off]
            }
            // Not supported compression/bitpix results in parsing the binary table as normal and thus this part is not reachable
            _ => unreachable!(),
        };
        self.desc.remaining_pixels -= 1;

        let v_f32 = value as f32;
        let v_scaled = v_f32 * self.desc.keywords.bscale + self.desc.keywords.bzero;

        Some(v_scaled as u8)
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
            let num_pixels = self.tile_pixel_count(row_idx) as u64;
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
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
                            let mut gz = GzDecoder::new(reader);
                            gz.read_exact(&mut buf[..])?;
                        }
                        // FIXME support bytepix
                        ZCmpType::Rice { blocksize, .. } => {
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
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
            ZCmpType::Rice { .. } => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                let off = 4 * idx;
                (self.buf[off] as i16) | ((self.buf[off + 1] as i16) << 8)
            }
            // Not supported compression/bitpix results in parsing the binary table as normal and thus this part is not reachable
            _ => unreachable!(),
        };
        self.desc.remaining_pixels -= 1;

        let v_f32 = value as f32;
        let v_scaled = v_f32 * self.desc.keywords.bscale + self.desc.keywords.bzero;

        Some(v_scaled as i16)
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
            let num_pixels = self.tile_pixel_count(row_idx) as u64;
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
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
                            let mut gz = GzDecoder::new(reader);
                            gz.read_exact(&mut buf[..])?;
                        }
                        // FIXME support bytepix
                        ZCmpType::Rice { blocksize, .. } => {
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
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

        let v_f32 = value as f32;
        let v_scaled = v_f32 * self.desc.keywords.bscale + self.desc.keywords.bzero;

        Some(v_scaled as i32)
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
            let num_pixels = self.tile_pixel_count(row_idx) as u64;
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
                        z_blank_idx,
                        z_blank,
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

            *scale = if let Some(z_scale_idx) = z_scale_idx {
                match row_data[*z_scale_idx] {
                    DataValue::Float { value, .. } => value,
                    DataValue::Double { value, .. } => value as f32,
                    _ => unreachable!(),
                }
            } else {
                1.0
            };

            *zero = if let Some(z_zero_idx) = z_zero_idx {
                match row_data[*z_zero_idx] {
                    DataValue::Float { value, .. } => value,
                    DataValue::Double { value, .. } => value as f32,
                    _ => unreachable!(),
                }
            } else {
                0.0
            };

            if let Some(idx) = z_blank_idx {
                *z_blank = match row_data[*idx] {
                    DataValue::UnsignedByte { value, .. } => Some(value as i32),
                    DataValue::Short { value, .. } => Some(value as i32),
                    DataValue::Integer { value, .. } => Some(value),
                    DataValue::Long { value, .. } => Some(value as i32),
                    _ => unreachable!(),
                }
            }

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
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
                            let mut gz = GzDecoder::new(reader);
                            gz.read_exact(&mut buf[..])?;
                        }
                        // FIXME support bytepix
                        ZCmpType::Rice { blocksize, .. } => {
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
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
            },
            ZCmpType::Gzip2 => {
                // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                // read from BigEndian, i.e. the most significant byte is at first and the least one is at last position
                let num_bytes = self.buf.len();
                let step_msb = num_bytes / 4;
                let bits = ((self.buf[idx] as u32) << 24)
                    | ((self.buf[idx + step_msb] as u32) << 16)
                    | ((self.buf[idx + 2 * step_msb] as u32) << 8)
                    | (self.buf[idx + 3 * step_msb] as u32);

                self.desc.keywords.unquantize(bits as i32)
            }
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

impl<R> Iterator for It<R, F64Keywords>
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
            let num_pixels = self.tile_pixel_count(row_idx) as u64;
            self.desc.n_pixels = num_pixels;
            self.desc.remaining_pixels = num_pixels;

            let TileDesc {
                keywords:
                    F64Keywords {
                        z_scale_idx,
                        z_zero_idx,
                        scale,
                        zero,
                        z_dither_0,
                        z_quantiz,
                        quantiz,
                        z_blank_idx,
                        z_blank,
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

            *scale = if let Some(z_scale_idx) = z_scale_idx {
                match row_data[*z_scale_idx] {
                    DataValue::Float { value, .. } => value,
                    DataValue::Double { value, .. } => value as f32,
                    _ => unreachable!(),
                }
            } else {
                1.0
            };

            *zero = if let Some(z_zero_idx) = z_zero_idx {
                match row_data[*z_zero_idx] {
                    DataValue::Float { value, .. } => value,
                    DataValue::Double { value, .. } => value as f32,
                    _ => unreachable!(),
                }
            } else {
                0.0
            };

            if let Some(idx) = z_blank_idx {
                *z_blank = match row_data[*idx] {
                    DataValue::UnsignedByte { value, .. } => Some(value as i32),
                    DataValue::Short { value, .. } => Some(value as i32),
                    DataValue::Integer { value, .. } => Some(value),
                    DataValue::Long { value, .. } => Some(value as i32),
                    _ => unreachable!(),
                }
            }

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
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
                            let mut gz = GzDecoder::new(reader);
                            gz.read_exact(&mut buf[..])?;
                        }
                        // FIXME support bytepix
                        ZCmpType::Rice { blocksize, .. } => {
                            let tile_bytes = num_pixels as usize * std::mem::size_of::<u32>();
                            buf.resize(tile_bytes, 0);
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
