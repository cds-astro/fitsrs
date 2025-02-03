use std::fmt::Debug;
use crate::hdu::data::{AsyncDataBufRead, stream::St};
use crate::hdu::header::Xtension;
use crate::hdu::header::extension::bintable::{ArrayDescriptorTy, TForm, TileCompressedImageTy, VariableArrayTy, A, B, C, D, E, I, J, K, L, M, P, Q, X, TileCompressedImage};

use byteorder::BigEndian;
use flate2::read::GzDecoder;
use crate::hdu::header::extension::bintable::{BinTable, TFormType};
use crate::hdu::FitsRead;
use log::warn;
use std::io::{Bytes, SeekFrom};
use std::io::Read;

impl<'a, R> FitsRead<'a, BinTable> for R
where
    R: Read + Debug + 'a,
{
    type Data = TableData<&'a mut Self>;

    fn read_data_unit(&'a mut self, ctx: &BinTable, start_pos: u64) -> Self::Data
        where
            Self: Sized {
        TableData::new(self, ctx, start_pos)
    }
}

/// A reader that can overload another reader when the necessity comes to
/// 
/// When parsing tile compressed images, we might need to overload the current reader with a Gzip/RICE decoder
#[derive(Debug)]
enum DataReaderState {
    /// Reader on the main table
    MainTable,
    /// Reader on the HEAP.
    /// FITS tile compressed image convention introduce that tile can be encoded
    /// Therefore we decorate our reader
    HEAP {
        /// The position of the reader before leaving the main table
        /// This will be used to go back to the main table reading state
        main_table_pos: SeekFrom,
        /// The type contained in the heap that we are reading
        ty: ArrayDescriptorTy,
        /// The number of bytes remaining to read
        num_bytes_to_read: u64,
        /// The number of elements to read
        n_elems: u64,
        /// The size in bytes of one element
        t_byte_size: u64,
    }
}

use crate::error::Error;

use byteorder::ReadBytesExt;

#[derive(Debug)]
pub struct TableData<R> {
    /// The reader owned
    pub reader: R,
    /// An intern state enum variable telling if we are parsing the main table or the heap
    state: DataReaderState,

    /// Columns ids to return the values, by default all fields are taken into account
    pub cols_idx: Vec<usize>,
    /// A byte offset to seek 
    pub col_byte_offsets: Vec<usize>,
    /// Context of the binary table it contains all the mandatory and optional cards parsed from the header unit
    pub ctx: BinTable,

    /// Col index currently read
    col_idx: usize,
    /// Item index currently read in the cur col read
    item_idx: usize,
    /// A byte offset from the beginning of the main data table to check if we read all the bin table
    byte_offset: usize,
    /// The total number of bytes in the main data table
    main_data_table_byte_size: usize,
    /// Start byte position of the data unit
    start_pos: u64,

    /// buffer for storing uncompressed data from GZIP/RICE
    buf: Vec<u8>,
    /// current row index
    row_idx: usize,

    /// When storing tile compressed images, RICE asks for a number of block (i.e. a number of pixels) to process
    /// Default value is 32 but in some fits, the true nblock can be stored in ZNAME/ZVAL. 
    nblock: i32,
}

impl<R> TableData<Cursor<R>>
where 
    R: AsRef<[u8]>
{
    /// For in memory buffers, access the raw bytes of the main data table + HEAP
    pub fn raw_bytes(&self) -> &[u8] {
        let inner = self.reader.get_ref();
        let raw_bytes = inner.as_ref();

        let s = self.start_pos as usize;
        let e = s + (self.ctx.get_num_bytes_data_block() as usize);
        &raw_bytes[s..e]
    }
}

use std::io::Cursor;
/// Get a reference to the inner reader for in-memory readers
impl<R> TableData<Cursor<R>> {
    pub const fn get_ref(&self) -> &R {
        self.reader.get_ref()
    }
}

use std::io::Take;
impl<R> TableData<R>
where 
    R: Read
{
    /// Gives an iterator over the bytes of the main data table
    pub fn bytes(self) -> Bytes<Take<R>> {
        let only_main_data_table = self.reader.take(self.main_data_table_byte_size as u64);
        only_main_data_table.bytes()
    }
}

impl<R> TableData<R> {
    pub fn new(
        reader: R,
        ctx: &BinTable,
        start_pos: u64
    ) -> Self {
        let state = DataReaderState::MainTable;

        // Compute an byte offset for each columns to know at which byte index does the column
        // starts inside a row
        let mut cumul_num_bytes = 0;
        let col_byte_offsets = ctx.tforms
            .iter()
            .map(|tform| {
                let col_byte_offset = cumul_num_bytes;
                cumul_num_bytes += tform.num_bytes_field();
                col_byte_offset
            }).collect();

        let cols_idx = (0..(ctx.tforms.len())).collect();

        let col_idx = 0;
        let item_idx = 0;

        let main_data_table_byte_size = (ctx.naxis1 * ctx.naxis2) as usize;
        let byte_offset = 0;

        // This buffer is only used if tile compressed image in the gzip compression is to be found
        let mut buf = vec![];

        // Allocation of a buffer at init of the iterator that is the size of the biggest tiles we can found
        // on the tile compressed image. This is simply given by the ZTILEi keyword.
        // Some tiles found on the border of the image can be smaller
        if let Some(TileCompressedImage { z_tilen, .. }) = &ctx.z_image {
            let n_elems_max = z_tilen.iter().fold(1, |mut tile_size, z_tilei| {
                tile_size *= *z_tilei;
                tile_size
            });

            // A little precision. The gzdecoder from flate2 seems to unzip data in a stream of u32 i.e. even if the type of data
            // to uncompress is byte or short, the result will be cast in a u32. I thus allocate for gzip compression, a buffer of
            // n_elems_max * size_in_bytes of u32.
            // It seems to be the same for RICE, so the latter rice decompression code is called on i32
            let num_bytes_max_tile = n_elems_max * std::mem::size_of::<u32>();

            buf.resize(dbg!(num_bytes_max_tile), 0);
        }

        let row_idx = 0;

        let nblock = 32;

        Self {
            reader,
            state,
            nblock,
            col_idx,
            item_idx,
            cols_idx,
            col_byte_offsets,
            byte_offset,
            main_data_table_byte_size,
            start_pos,
            ctx: ctx.clone(),
            buf,
            row_idx,
        }
    }

    /// Select fields to look, by default when not calling this method,
    /// all field values are returned
    pub fn select_fields(&mut self, cols: &[ColumnId]) -> &mut Self {
        self.cols_idx = cols.iter().filter_map(|col| {
            match col {
                ColumnId::Index(index) => {
                    // check that the index does not exceed the number of columns
                    if *index >= self.ctx.tforms.len() {
                        warn!("{} index provided exceeds the number of valid columns found in the table. This index will be discarded.", index);
                        None
                    } else {
                        Some(*index)
                    }
                },
                ColumnId::Name(name) => {
                    // If the column is given by its name, then we must search
                    // in the ttypes keywords to get its correct index
                    match self.ctx.ttypes.iter().position(|ttype| {
                        if let Some(ttype) = ttype {
                            ttype == name
                        } else {
                            false
                        }
                    }) {
                        Some(idx) => Some(idx),
                        None => {
                            warn!("{} field name has not been found. Its value is discarded", name);
                            None
                        }
                    }
                }
            }
        }).collect();

        self
    }

    /// This method allow the user to change the number of block that RICE decomp/comp algorithm must use.
    /// Default value is 32 but it may happen that fits provider use the ZNAME/ZVAL cards to provide another number of block
    pub fn set_rice_block_size(&mut self, nblock: u32) {
        self.nblock = nblock as i32;
    }
}

impl<R> TableData<R>
where 
    R: Seek
{
    fn seek_to_next_col(&mut self) -> Result<(), Error> {
        // detect if we jump of row
        if self.col_idx == self.cols_idx.len() - 1 {
            self.row_idx += 1;
        }

        self.col_idx = (self.col_idx + 1) % self.cols_idx.len();
        self.item_idx = 0;

        let col_idx = self.cols_idx[self.col_idx];

        let next_row_byte_off = self.col_byte_offsets[col_idx] as i64;
        let cur_row_byte_off = (self.byte_offset as i64) % (self.ctx.naxis1 as i64);

        let off = if next_row_byte_off > cur_row_byte_off {
            // next col is on the same row
            next_row_byte_off - cur_row_byte_off
        } else if next_row_byte_off < cur_row_byte_off {
            // next col is on the next row
            // get to the end of the current row
            (self.ctx.naxis1 as i64) - cur_row_byte_off
            // add the off the the start of the next row
                + next_row_byte_off
        } else {
            // we are at the good location where the next col is
            0
        };

        // seek to the next col location
        self.reader.seek_relative(off)?;

        Ok(())
    }

    /// Seek directly to a specific row idx
    /// 
    /// # Params
    /// * `idx` - Index of the row to jump to
    pub fn seek_to_row(&mut self, idx: usize) -> Result<(), Error> {
        if idx >= self.ctx.naxis2 as usize {
            Err(Error::StaticError("The row index specified is > than the number of rows of the table"))
        } else {
            self.col_idx = 0;
            self.row_idx = idx;
            self.item_idx = 0;

            let new_byte_offset =
                // go to the beginning of the idx-th row
                (idx as i64) * (self.ctx.naxis1 as i64)
                // go to the first column of that row
                + (self.col_byte_offsets[self.cols_idx[0]] as i64);

            // Offset for moving from the current position to the new row position
            let off =
                // go back to the beginning of the main table data block
                - (self.byte_offset as i64) + new_byte_offset;

            let _ = self.reader.seek_relative(off)?;

            self.byte_offset = new_byte_offset as usize;

            Ok(())
        }
    }

    /// Jump to the heap at a specific offset in the HEAP associated to the binary table
    /// 
    /// This method takes the ownership to change the state of it
    /// 
    /// * Params
    /// 
    /// reader - A reference to the reader
    /// ctx - The context will give some properties (i.e. location of the heap)
    /// byte_offset_from_main_table - a byte offset where the reader is in the main table
    /// ty - Description of the array stored in the heap
    /// byte_offset - byte offset extracted in the row indicating where the start of the heap region occurs
    /// n_elems - number of elements extracted in the row indicating how many elements the array contains
    /// t_byte_size - the size in bytes of an element inside the array
    fn jump_to_heap(
        &mut self,
        ty: ArrayDescriptorTy,
        byte_offset: u64,
        mut n_elems: u64,
        mut t_byte_size: u64,
    ) -> Result<(), Error>
    where 
        R: Read + Seek,
    {
        match self.state {
            // We are already in the heap, we do nothing
            DataReaderState::HEAP { .. } => (),
            DataReaderState::MainTable => {
                // Move to the HEAP
                let off =
                    // go back to the beginning of the main table data block
                    - (self.byte_offset as i64)
                    // from the beginning of the main table go to the beginning of the heap
                    + self.ctx.theap as i64
                    // from the beginning of the heap go to the start of the array
                    + byte_offset as i64;

                // Get the current location used to go back to the main table
                let pos = SeekFrom::Start(self.reader.stream_position()?);

                // Go to the HEAP
                let _ = self.reader.seek_relative(off)?;

                // In case this variable length column refers to a tile compressed image
                if let (ArrayDescriptorTy::TileCompressedImage(arr_desc), Some(tci)) = (ty, &self.ctx.z_image) {
                    n_elems = tci.tile_size_from_row_idx(self.row_idx).iter().fold(1, |mut n, &tile| {
                        n *= tile;
                        n
                    }) as u64;
                    t_byte_size = tci.z_bitpix.byte_size() as u64;

                    match arr_desc {
                        TileCompressedImageTy::Gzip1U8 | TileCompressedImageTy::Gzip1I16 | TileCompressedImageTy::Gzip1I32 => {
                            let mut gz = GzDecoder::new(&mut self.reader);
                            gz.read_exact(&mut self.buf[..])?;
                        },
                        TileCompressedImageTy::RiceU8 => {
                            let mut rice = RICEDecoder::<_, i32>::new(&mut self.reader, self.nblock, n_elems as i32);
                            rice.read_exact(&mut self.buf[..])?;
                        }
                        TileCompressedImageTy::RiceI16 => {
                            let mut rice = RICEDecoder::<_, i32>::new(&mut self.reader, self.nblock, n_elems as i32);
                            rice.read_exact(&mut self.buf[..])?;
                        }
                        TileCompressedImageTy::RiceI32 => {
                            let mut rice = RICEDecoder::<_, i32>::new(&mut self.reader, self.nblock, n_elems as i32);
                            rice.read_exact(&mut self.buf[..])?;
                        }
                    }
                }

                let num_bytes_to_read = n_elems * t_byte_size;
                self.state = DataReaderState::HEAP { main_table_pos: pos, ty, num_bytes_to_read, t_byte_size, n_elems };
            }
        }

        Ok(())
    }

    fn jump_to_main_table(&mut self) -> Result<(), Error>
    where 
        R: Seek
    {
        match self.state {
            DataReaderState::HEAP { main_table_pos, ..} => {
                // move back to the main table
                let _ = self.reader.seek(main_table_pos)?;

                self.state = DataReaderState::MainTable;
            },
            DataReaderState::MainTable => ()
        }

        Ok(())
    }
}

impl<R> TableData<R>
where
    R: Read + Seek + Debug
{
    pub fn row_iter(self) -> impl Iterator<Item = Box<[DataValue]>> {
        TableRowData::new(self)
    }
}

struct TableRowData<R> {
    data: TableData<R>,
    idx_row: usize,
}

impl<R> TableRowData<R> {
    fn new(data: TableData<R>) -> Self {
        Self {
            data,
            idx_row: 0,
        }
    }
}

impl<R> Iterator for TableRowData<R>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = Box<[DataValue]>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut row_data = vec![];

        while self.data.row_idx == self.idx_row {
            row_data.push(self.data.next()?);
        }

        self.idx_row += 1;

        Some(row_data.into_boxed_slice())
    }
}


use std::io::Seek;
use super::rice::RICEDecoder;
use super::{ColumnId, DataValue};
impl<R> Iterator for TableData<R>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = DataValue;

    fn next(&mut self) -> Option<Self::Item> {
        // First get the column index in the main data table where the reader is
        let col_idx = self.cols_idx[self.col_idx];

        match &mut self.state {
            DataReaderState::HEAP { ty, num_bytes_to_read, n_elems, t_byte_size, .. } => {
                // We will build an iterator that will parse the variable length array
                // in the heap
                // Here, our reader is at the good heap location

                // idx of the elem in the heap area
                let idx = (*n_elems - (*num_bytes_to_read) / (*t_byte_size)) as usize;

                let value = match *ty {
                    // GZIP compression
                    // Unsigned byte
                    ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::Gzip1U8) => {
                        *num_bytes_to_read -= B::BYTES_SIZE as u64;
                        // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                        let off = 4 * idx;
                        let value = i32::from_be_bytes([self.buf[off], self.buf[off + 1], self.buf[off + 2], self.buf[off + 3]]) as u8;
                        DataValue::UnsignedByte { value, column: ColumnId::Index(col_idx), idx  }
                    }
                    // 16-bit integer
                    ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::Gzip1I16) => {
                        *num_bytes_to_read -= I::BYTES_SIZE as u64;
                        // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                        let off = 4 * idx;
                        let value = i32::from_be_bytes([self.buf[off], self.buf[off + 1], self.buf[off + 2], self.buf[off + 3]]) as i16;
                        DataValue::Short { value, column: ColumnId::Index(col_idx), idx  }
                    }
                    // 32-bit integer
                    ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::Gzip1I32) => {
                        *num_bytes_to_read -= J::BYTES_SIZE as u64;
                        // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                        let off = 4 * idx;
                        let value = i32::from_be_bytes([self.buf[off], self.buf[off + 1], self.buf[off + 2], self.buf[off + 3]]);
                        DataValue::Integer { value, column: ColumnId::Index(col_idx), idx  }
                    }
                    // RICE compression
                    // Unsigned byte
                    ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::RiceU8) => {
                        *num_bytes_to_read -= B::BYTES_SIZE as u64;
                        // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                        let off = 4 * idx;
                        let value = self.buf[off];
                        DataValue::UnsignedByte { value, column: ColumnId::Index(col_idx), idx  }
                    }
                    // 16-bit integer
                    ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::RiceI16) => {
                        *num_bytes_to_read -= I::BYTES_SIZE as u64;
                        // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                        let off = 4 * idx;
                        let value = (self.buf[off] as i16) | ((self.buf[off + 1] as i16) << 8);
                        DataValue::Short { value, column: ColumnId::Index(col_idx), idx  }
                    }
                    // 32-bit integer
                    ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::RiceI32) => {
                        *num_bytes_to_read -= J::BYTES_SIZE as u64;
                        // We need to get the byte index in the buffer storing u32, i.e. 4 bytes per elements
                        let off = 4 * idx;
                        let value = i32::from_ne_bytes([self.buf[off], self.buf[off + 1], self.buf[off + 2], self.buf[off + 3]]);
                        DataValue::Integer { value, column: ColumnId::Index(col_idx), idx  }
                    }
                    // No compressed data
                    ArrayDescriptorTy::Default(va_ty) => {
                        match va_ty {
                            VariableArrayTy::L => {
                                let value = self.reader.read_u8().ok()? != 0;
                                *num_bytes_to_read -= L::BYTES_SIZE as u64;
                                DataValue::Logical { value, column: ColumnId::Index(col_idx), idx  }
                            },
                            VariableArrayTy::X => {
                                let byte = self.reader.read_u8().ok()?;
                                *num_bytes_to_read -= X::BYTES_SIZE as u64;
                                DataValue::Bit { byte, bit_idx: 0, column: ColumnId::Index(col_idx), idx  }
                            },
                            VariableArrayTy::B => {
                                let value = self.reader.read_u8().ok()?;
                                *num_bytes_to_read -= B::BYTES_SIZE as u64;
                                DataValue::UnsignedByte { value, column: ColumnId::Index(col_idx), idx  }
                            }
                            VariableArrayTy::I => {
                                let value = self.reader.read_i16::<BigEndian>().ok()?;
                                *num_bytes_to_read -= I::BYTES_SIZE as u64;
                                DataValue::Short { value, column: ColumnId::Index(col_idx), idx  }
                            }
                            VariableArrayTy::J => {
                                let value = self.reader.read_i32::<BigEndian>().ok()?;
                                *num_bytes_to_read -= J::BYTES_SIZE as u64;
                                DataValue::Integer { value, column: ColumnId::Index(col_idx), idx  }
                            }
                            VariableArrayTy::K => {
                                let value = self.reader.read_i64::<BigEndian>().ok()?;
                                *num_bytes_to_read -= K::BYTES_SIZE as u64;
                                DataValue::Long { value, column: ColumnId::Index(col_idx), idx  }
                            }
                            VariableArrayTy::A => {
                                let value = self.reader.read_u8().ok()? as char;
                                *num_bytes_to_read -= A::BYTES_SIZE as u64;
                                DataValue::Character { value, column: ColumnId::Index(col_idx), idx  }
                            }
                            VariableArrayTy::E => {
                                let value = self.reader.read_f32::<BigEndian>().ok()?;
                                *num_bytes_to_read -= E::BYTES_SIZE as u64;
                                DataValue::Float { value, column: ColumnId::Index(col_idx), idx  }
                            }
                            VariableArrayTy::D => {
                                let value = self.reader.read_f64::<BigEndian>().ok()?;
                                *num_bytes_to_read -= D::BYTES_SIZE as u64;
                                DataValue::Double { value, column: ColumnId::Index(col_idx), idx  }
                            }
                            VariableArrayTy::C => {
                                let real = self.reader.read_f32::<BigEndian>().ok()?;
                                let imag = self.reader.read_f32::<BigEndian>().ok()?;

                                *num_bytes_to_read -= C::BYTES_SIZE as u64;
                                DataValue::ComplexFloat { real, imag, column: ColumnId::Index(col_idx), idx  }
                            }
                            VariableArrayTy::M => {
                                let real = self.reader.read_f64::<BigEndian>().ok()?;
                                let imag = self.reader.read_f64::<BigEndian>().ok()?;

                                *num_bytes_to_read -= M::BYTES_SIZE as u64;
                                DataValue::ComplexDouble { real, imag, column: ColumnId::Index(col_idx), idx  }
                            }
                        }
                    }
                };

                if *num_bytes_to_read == 0 {
                    // no more bytes to read on the heap.
                    // we first jump back to the main table where we were
                    self.jump_to_main_table().ok()?;
                    // and we seek the next column there
                    self.seek_to_next_col().ok()?;
                }

                Some(value)
            },
            DataReaderState::MainTable => {
                // Check whether we are at the end of the main data table
                // This would mean we read all the table
                // Read the next value

                if self.byte_offset == self.main_data_table_byte_size {
                    None
                } else {
                    // Retrieve its tform to know which type to read from the reader
                    match &self.ctx.tforms[col_idx] {
                        // Logical
                        TFormType::L { repeat_count } => {
                            let byte = self.reader.read_u8().ok()?;
                            self.byte_offset += L::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Logical { value: byte != 0, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // Bit
                        TFormType::X { repeat_count } => {
                            let byte = self.reader.read_u8().ok()?;
                            self.byte_offset += X::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == (*repeat_count / 8) + 1 {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Bit { byte, bit_idx: 0, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // Unsigned byte
                        TFormType::B { repeat_count } => {
                            let byte = self.reader.read_u8().ok()?;
                            self.byte_offset += B::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::UnsignedByte { value: byte, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // 16-bit integer
                        TFormType::I { repeat_count } => {
                            let short = self.reader.read_i16::<BigEndian>().ok()?;
                            self.byte_offset += I::BYTES_SIZE;
        
                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }
        
                            Some(DataValue::Short { value: short, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // 32-bit integer
                        TFormType::J { repeat_count } => {
                            let int = self.reader.read_i32::<BigEndian>().ok()?;
                            self.byte_offset += J::BYTES_SIZE;
        
                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }
        
                            Some(DataValue::Integer { value: int, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // 64-bit integer
                        TFormType::K { repeat_count } => {
                            let long = self.reader.read_i64::<BigEndian>().ok()?;
                            self.byte_offset += K::BYTES_SIZE;
        
                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }
        
                            Some(DataValue::Long { value: long, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // Character
                        TFormType::A { repeat_count } => {
                            let c = self.reader.read_u8().ok()?;
                            self.byte_offset += A::BYTES_SIZE;
        
                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }
        
                            Some(DataValue::Character { value: c as char, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // Single-precision floating point
                        TFormType::E { repeat_count } => {
                            let float = self.reader.read_f32::<BigEndian>().ok()?;
                            self.byte_offset += E::BYTES_SIZE;
        
                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }
        
                            Some(DataValue::Float { value: float, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // Double-precision floating point
                        TFormType::D { repeat_count } => {
                            let double = self.reader.read_f64::<BigEndian>().ok()?;
                            self.byte_offset += D::BYTES_SIZE;
        
                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }
        
                            Some(DataValue::Double { value: double, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // Single-precision complex
                        TFormType::C { repeat_count } => {
                            let real = self.reader.read_f32::<BigEndian>().ok()?;
                            let imag = self.reader.read_f32::<BigEndian>().ok()?;
                            self.byte_offset += C::BYTES_SIZE;
        
                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }
        
                            Some(DataValue::ComplexFloat { real, imag, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // Double-precision complex
                        TFormType::M { repeat_count } => {
                            let real = self.reader.read_f64::<BigEndian>().ok()?;
                            let imag = self.reader.read_f64::<BigEndian>().ok()?;
                            self.byte_offset += M::BYTES_SIZE;
        
                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }
        
                            Some(DataValue::ComplexDouble { real, imag, column: ColumnId::Index(col_idx), idx: self.item_idx - 1 }) // Determine the count idx inside the field
                        },
                        // Array Descriptor (32-bit) 
                        TFormType::P { ty, t_byte_size, .. } => {
                            let n_elems = self.reader.read_u32::<BigEndian>().ok()?;
                            let byte_offset = self.reader.read_u32::<BigEndian>().ok()?;
        
                            self.byte_offset += P::BYTES_SIZE;
        
                            self.jump_to_heap(
                                *ty,
                                byte_offset as u64,
                                n_elems as u64,
                                *t_byte_size,
                            ).ok()?;
        
                            self.next()
                        },
                        // Array Descriptor (64-bit) 
                        TFormType::Q { ty, t_byte_size, .. } => {
                            let n_elems = self.reader.read_u64::<BigEndian>().ok()?;
                            let byte_offset = self.reader.read_u64::<BigEndian>().ok()?;
        
                            self.byte_offset += Q::BYTES_SIZE;
        
                            self.jump_to_heap(
                                *ty,
                                byte_offset as u64,
                                n_elems as u64,
                                *t_byte_size,
                            ).ok()?;
        
                            self.next()
                        },
                    }
                }
            }
        }
    }
}

use async_trait::async_trait;
use futures::AsyncReadExt;
use std::marker::Unpin;

#[async_trait(?Send)]
impl<'a, R> AsyncDataBufRead<'a, BinTable> for futures::io::BufReader<R>
where
    R: AsyncReadExt + Debug + Unpin + 'a,
{
    type Data = St<'a, Self, u8>;

    fn prepare_data_reading(
        _ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        St::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}