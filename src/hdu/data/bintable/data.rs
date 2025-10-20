use crate::hdu::data::{stream::St, AsyncDataBufRead};
use crate::hdu::header::extension::bintable::{
    TForm, VariableArrayTy, A, B, C, D, E, I, J, K, L, M, P, Q, X,
};
use crate::hdu::header::{Header, Xtension};
use std::fmt::Debug;

use super::row::TableRowData;
use crate::hdu::header::extension::bintable::{BinTable, TFormType};
use crate::hdu::FitsRead;
use byteorder::BigEndian;
use log::warn;
use std::io::SeekFrom;
use std::io::{BufReader, Read};

impl<'a, R> FitsRead<'a, BinTable> for R
where
    R: Read + Debug + 'a,
{
    type Data = BinaryTableData<&'a mut Self>;

    fn read_data_unit(&'a mut self, header: &Header<BinTable>, start_pos: u64) -> Self::Data
    where
        Self: Sized,
    {
        BinaryTableData::new(self, header, start_pos)
    }
}

#[derive(Debug)]
pub enum BinaryTableData<R> {
    Table(TableData<R>),
    TileCompressed(Pixels<R>),
}

/*
impl<R> Iterator for BinaryTableData<R>
where
    R: Debug + Seek + Read,
{
    type Item = DataValue;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Table(data) => data.next(),
            Self::TileCompressed(data) => data.next(),
        }
    }
}
*/

use super::tile_compressed::pixels::Pixels;
impl<R> BinaryTableData<R>
where
    R: Debug + Read,
{
    fn new(reader: R, header: &Header<BinTable>, start_pos: u64) -> Self {
        let ctx = header.get_xtension();
        let data = TableData::new(reader, header, start_pos);

        if let Some(tile_compressed) = &ctx.z_image {
            BinaryTableData::TileCompressed(Pixels::new(data, header, tile_compressed))
        } else {
            BinaryTableData::Table(data)
        }
    }

    pub fn table_data(self) -> TableData<R> {
        match self {
            BinaryTableData::TileCompressed(pixels) => match pixels {
                Pixels::U8(pixels) => pixels.row_it.table_data(),
                Pixels::I16(pixels) => pixels.row_it.table_data(),
                Pixels::I32(pixels) => pixels.row_it.table_data(),
                Pixels::F32(pixels) => pixels.row_it.table_data(),
                Pixels::F64(pixels) => pixels.row_it.table_data(),
            },
            BinaryTableData::Table(table) => table,
        }
    }
}

impl<R> BinaryTableData<R> {
    pub fn row_iter(self) -> TableRowData<R> {
        match self {
            BinaryTableData::TileCompressed(pixels) => match pixels {
                Pixels::U8(pixels) => pixels.row_it,
                Pixels::I16(pixels) => pixels.row_it,
                Pixels::I32(pixels) => pixels.row_it,
                Pixels::F32(pixels) => pixels.row_it,
                Pixels::F64(pixels) => pixels.row_it,
            },
            BinaryTableData::Table(table) => table.row_iter(),
        }
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
        ty: VariableArrayTy,
        /// The number of bytes remaining to read
        num_bytes_to_read: u64,
        /// The number of elements to read
        n_elems: u64,
        /// The size in bytes of one element
        t_byte_size: u64,
    },
}

use crate::error::Error;

use byteorder::ReadBytesExt;

#[derive(Debug)]
pub struct TableData<R> {
    /// The reader owned
    pub(crate) reader: R,
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
    /// Seek to first col
    seek_to_first_col: bool,

    /// current row index, is read from the row iterator
    pub(crate) row_idx: usize,

    /// A flag indicating if the iterator will jump to the heap to parse the variable length array columns
    heap: bool,
}

impl<R> TableData<&mut Cursor<R>>
where
    R: AsRef<[u8]>,
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
impl<R> TableData<&mut Cursor<R>> {
    pub const fn get_ref(&self) -> &R {
        self.reader.get_ref()
    }
}

use std::io::Take;
impl<R> TableData<R>
where
    R: Read,
{
    /// Gives an iterator over the bytes of the main data table
    pub fn bytes(self) -> BufReader<Take<R>> {
        let only_main_data_table = self.reader.take(self.main_data_table_byte_size as u64);
        BufReader::new(only_main_data_table)
    }
}

impl<R> TableData<R> {
    pub fn new(reader: R, header: &Header<BinTable>, start_pos: u64) -> Self {
        let ctx = header.get_xtension();
        let state = DataReaderState::MainTable;

        // Compute an byte offset for each columns to know at which byte index does the column
        // starts inside a row
        let mut cumul_num_bytes = 0;
        let col_byte_offsets = ctx
            .tforms
            .iter()
            .map(|tform| {
                let col_byte_offset = cumul_num_bytes;
                cumul_num_bytes += tform.num_bytes_field();
                col_byte_offset
            })
            .collect();

        let cols_idx = (0..(ctx.tforms.len())).collect();

        let col_idx = 0;
        let item_idx = 0;

        let main_data_table_byte_size = (ctx.naxis1 * ctx.naxis2) as usize;
        let byte_offset = 0;

        let row_idx = 0;

        let heap = true;
        Self {
            reader,
            state,
            col_idx,
            item_idx,
            cols_idx,
            col_byte_offsets,
            seek_to_first_col: false,
            byte_offset,
            main_data_table_byte_size,
            start_pos,
            ctx: ctx.clone(),
            row_idx,
            heap,
        }
    }

    pub(crate) fn read_the_heap(&mut self, heap: bool) {
        self.heap = heap;
    }

    pub(crate) fn get_ctx(&self) -> &BinTable {
        &self.ctx
    }

    pub(crate) fn get_reader(&mut self) -> &mut R {
        &mut self.reader
    }

    /// Select fields to look, by default when not calling this method,
    /// all field values are returned
    pub fn select_fields(&mut self, cols: &[ColumnId]) -> &mut Self {
        self.cols_idx = cols
            .iter()
            .filter_map(|col| {
                match col {
                    ColumnId::Index(index) => {
                        // check that the index does not exceed the number of columns
                        if *index >= self.ctx.tforms.len() {
                            warn!("{index} index provided exceeds the number of valid columns found in the table. This index will be discarded.");
                            None
                        } else {
                            Some(*index)
                        }
                    },
                    ColumnId::Name(name) => {
                        // If the column is given by its name, then we must search
                        // in the ttypes keywords to get its correct index
                        self.ctx.find_field_by_ttype(name).or_else(|| {
                            warn!("{name} field name has not been found. Its value is discarded");
                            None
                        })
                    }
                }
            }
        ).collect();

        self.cols_idx.sort_unstable();

        // We must go to the first column at this point
        self.seek_to_first_col = true;

        self
    }
}

impl<R> TableData<R>
where
    R: Seek,
{
    fn seek_to_next_col(&mut self) -> Result<(), Error> {
        let last_col = self.col_idx == self.cols_idx.len() - 1;
        let last_row = self.row_idx == self.ctx.naxis2 as usize - 1;

        if last_col {
            self.row_idx += 1;
        }

        // If we are at the last row and the last col, then we go to the end of the main data table
        let off = if last_col && last_row {
            // seek to the end of the main data table
            self.main_data_table_byte_size as i64 - self.byte_offset as i64
        } else {
            self.col_idx = (self.col_idx + 1) % self.cols_idx.len();
            self.item_idx = 0;

            let col_idx = self.cols_idx[self.col_idx];

            let next_row_byte_off = self.col_byte_offsets[col_idx] as i64;
            let cur_row_byte_off = (self.byte_offset as i64) % (self.ctx.naxis1 as i64);

            let mut rel_row_byte_off = next_row_byte_off - cur_row_byte_off;

            if rel_row_byte_off < 0 {
                // next col is on the next row
                // get to the end of the current row
                rel_row_byte_off += self.ctx.naxis1 as i64;
            }

            rel_row_byte_off
        };

        self.reader.seek_relative(off)?;
        // update the byte offset as well
        self.byte_offset += off as usize;

        Ok(())
    }

    /// Seek directly to a specific row idx
    ///
    /// # Params
    /// * `idx` - Index of the row to jump to
    pub fn seek_to_row(&mut self, idx: usize) -> Result<(), Error> {
        if idx >= self.ctx.naxis2 as usize {
            Err(Error::StaticError(
                "The row index specified is > than the number of rows of the table",
            ))
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

            self.reader.seek_relative(off)?;

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
    pub(crate) fn jump_to_heap(
        &mut self,
        ty: VariableArrayTy,
        byte_offset: u64,
        n_elems: u64,
        t_byte_size: u64,
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
                self.reader.seek_relative(off)?;

                let num_bytes_to_read = n_elems * t_byte_size;
                self.state = DataReaderState::HEAP {
                    ty,
                    main_table_pos: pos,
                    num_bytes_to_read,
                    t_byte_size,
                    n_elems,
                };
            }
        }

        Ok(())
    }

    pub(crate) fn jump_to_main_table(&mut self) -> Result<(), Error>
    where
        R: Seek,
    {
        match self.state {
            DataReaderState::HEAP { main_table_pos, .. } => {
                // move back to the main table
                let _ = self.reader.seek(main_table_pos)?;

                self.state = DataReaderState::MainTable;
            }
            DataReaderState::MainTable => (),
        }

        Ok(())
    }
}

impl<R> TableData<R> {
    pub fn row_iter(self) -> TableRowData<R> {
        TableRowData::new(self)
    }
}

use super::{ColumnId, DataValue};
use std::io::Seek;
impl<R> Iterator for TableData<R>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = DataValue;

    fn next(&mut self) -> Option<Self::Item> {
        // First get the column index in the main data table where the reader is
        let col_idx = self.cols_idx[self.col_idx];

        // a select fields has been done, we must offset to the first col
        if self.seek_to_first_col {
            self.seek_to_first_col = false;

            let off = self.col_byte_offsets[col_idx] as i64;
            // seek to the next col location
            self.reader.seek_relative(off).ok()?;
            // update the byte offset as well
            self.byte_offset += off as usize;
        }

        match &mut self.state {
            DataReaderState::HEAP {
                ty,
                num_bytes_to_read,
                n_elems,
                t_byte_size,
                ..
            } => {
                // We will build an iterator that will parse the variable length array
                // in the heap
                // Here, our reader is at the good heap location

                // idx of the elem in the heap area
                let idx = (*n_elems - (*num_bytes_to_read) / (*t_byte_size)) as usize;

                let value = match ty {
                    VariableArrayTy::L => {
                        let value = self.reader.read_u8().ok()? != 0;
                        *num_bytes_to_read -= L::BYTES_SIZE as u64;
                        DataValue::Logical {
                            value,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::X => {
                        let byte = self.reader.read_u8().ok()?;
                        *num_bytes_to_read -= X::BYTES_SIZE as u64;
                        DataValue::Bit {
                            byte,
                            bit_idx: 0,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::B => {
                        let value = self.reader.read_u8().ok()?;
                        *num_bytes_to_read -= B::BYTES_SIZE as u64;
                        DataValue::UnsignedByte {
                            value,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::I => {
                        let value = self.reader.read_i16::<BigEndian>().ok()?;
                        *num_bytes_to_read -= I::BYTES_SIZE as u64;
                        DataValue::Short {
                            value,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::J => {
                        let value = self.reader.read_i32::<BigEndian>().ok()?;
                        *num_bytes_to_read -= J::BYTES_SIZE as u64;
                        DataValue::Integer {
                            value,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::K => {
                        let value = self.reader.read_i64::<BigEndian>().ok()?;
                        *num_bytes_to_read -= K::BYTES_SIZE as u64;
                        DataValue::Long {
                            value,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::A => {
                        let value = self.reader.read_u8().ok()? as char;
                        *num_bytes_to_read -= A::BYTES_SIZE as u64;
                        DataValue::Character {
                            value,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::E => {
                        let value = self.reader.read_f32::<BigEndian>().ok()?;
                        *num_bytes_to_read -= E::BYTES_SIZE as u64;
                        DataValue::Float {
                            value,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::D => {
                        let value = self.reader.read_f64::<BigEndian>().ok()?;
                        *num_bytes_to_read -= D::BYTES_SIZE as u64;
                        DataValue::Double {
                            value,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::C => {
                        let real = self.reader.read_f32::<BigEndian>().ok()?;
                        let imag = self.reader.read_f32::<BigEndian>().ok()?;

                        *num_bytes_to_read -= C::BYTES_SIZE as u64;
                        DataValue::ComplexFloat {
                            real,
                            imag,
                            column: ColumnId::Index(col_idx),
                            idx,
                        }
                    }
                    VariableArrayTy::M => {
                        let real = self.reader.read_f64::<BigEndian>().ok()?;
                        let imag = self.reader.read_f64::<BigEndian>().ok()?;

                        *num_bytes_to_read -= M::BYTES_SIZE as u64;
                        DataValue::ComplexDouble {
                            real,
                            imag,
                            column: ColumnId::Index(col_idx),
                            idx,
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
            }
            DataReaderState::MainTable => {
                // Check whether we are at the end of the main data table
                // This would mean we read all the table
                // Read the next value

                if self.byte_offset == self.main_data_table_byte_size {
                    None
                } else {
                    let idx = self.item_idx;

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

                            Some(DataValue::Logical {
                                value: byte != 0,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // Bit
                        TFormType::X { repeat_count } => {
                            let byte = self.reader.read_u8().ok()?;
                            self.byte_offset += X::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == (*repeat_count / 8) + 1 {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Bit {
                                byte,
                                bit_idx: 0,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // Unsigned byte
                        TFormType::B { repeat_count } => {
                            let byte = self.reader.read_u8().ok()?;
                            self.byte_offset += B::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::UnsignedByte {
                                value: byte,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // 16-bit integer
                        TFormType::I { repeat_count } => {
                            let short = self.reader.read_i16::<BigEndian>().ok()?;
                            self.byte_offset += I::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Short {
                                value: short,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // 32-bit integer
                        TFormType::J { repeat_count } => {
                            let int = self.reader.read_i32::<BigEndian>().ok()?;
                            self.byte_offset += J::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Integer {
                                value: int,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // 64-bit integer
                        TFormType::K { repeat_count } => {
                            let long = self.reader.read_i64::<BigEndian>().ok()?;
                            self.byte_offset += K::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Long {
                                value: long,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // Character
                        TFormType::A { repeat_count } => {
                            let c = self.reader.read_u8().ok()?;
                            self.byte_offset += A::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Character {
                                value: c as char,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // Single-precision floating point
                        TFormType::E { repeat_count } => {
                            let float = self.reader.read_f32::<BigEndian>().ok()?;
                            self.byte_offset += E::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Float {
                                value: float,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // Double-precision floating point
                        TFormType::D { repeat_count } => {
                            let double = self.reader.read_f64::<BigEndian>().ok()?;
                            self.byte_offset += D::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::Double {
                                value: double,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // Single-precision complex
                        TFormType::C { repeat_count } => {
                            let real = self.reader.read_f32::<BigEndian>().ok()?;
                            let imag = self.reader.read_f32::<BigEndian>().ok()?;
                            self.byte_offset += C::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::ComplexFloat {
                                real,
                                imag,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // Double-precision complex
                        TFormType::M { repeat_count } => {
                            let real = self.reader.read_f64::<BigEndian>().ok()?;
                            let imag = self.reader.read_f64::<BigEndian>().ok()?;
                            self.byte_offset += M::BYTES_SIZE;

                            self.item_idx += 1;
                            if self.item_idx == *repeat_count {
                                self.seek_to_next_col().ok()?;
                            }

                            Some(DataValue::ComplexDouble {
                                real,
                                imag,
                                column: ColumnId::Index(col_idx),
                                idx,
                            }) // Determine the count idx inside the field
                        }
                        // Array Descriptor (32-bit)
                        TFormType::P {
                            ty, t_byte_size, ..
                        } => {
                            let num_elems = self.reader.read_u32::<BigEndian>().ok()?;
                            let offset_byte = self.reader.read_u32::<BigEndian>().ok()?;

                            self.byte_offset += P::BYTES_SIZE;

                            if self.heap {
                                self.jump_to_heap(
                                    *ty,
                                    offset_byte as u64,
                                    num_elems as u64,
                                    *t_byte_size,
                                )
                                .ok()?;

                                self.next()
                            } else {
                                // We need to seek to the next call if we do not jump to the heap, notifying
                                // we finished parsing this field
                                self.seek_to_next_col().ok()?;

                                // just returns the n_elems and offset from the iterator
                                Some(DataValue::VariableLengthArray32 {
                                    num_elems,
                                    offset_byte,
                                })
                            }
                        }
                        // Array Descriptor (64-bit)
                        TFormType::Q {
                            ty, t_byte_size, ..
                        } => {
                            let num_elems = self.reader.read_u64::<BigEndian>().ok()?;
                            let offset_byte = self.reader.read_u64::<BigEndian>().ok()?;

                            self.byte_offset += Q::BYTES_SIZE;

                            if self.heap {
                                self.jump_to_heap(*ty, offset_byte, num_elems, *t_byte_size)
                                    .ok()?;

                                self.next()
                            } else {
                                // We need to seek to the next call if we do not jump to the heap, notifying
                                // we finished parsing this field
                                self.seek_to_next_col().ok()?;

                                // just returns the n_elems and offset from the iterator
                                Some(DataValue::VariableLengthArray64 {
                                    num_elems,
                                    offset_byte,
                                })
                            }
                        }
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
