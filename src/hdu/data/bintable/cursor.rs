use std::fmt::Debug;

use crate::hdu::header::extension::bintable::{P, Q, ArrayDescriptor};

use byteorder::{BigEndian, ByteOrder};

use crate::hdu::header::extension::bintable::{BinTable, TFormType};
use crate::hdu::FitsRead;
use crate::hdu::header::extension::Xtension;
use std::io::Cursor;
/*
use super::FieldTy;

#[derive(Debug)]
pub struct RowBytesIt<'a> {
    // The bytes over the whole data block
    pub bytes: &'a [u8],

    num_bytes_in_row: usize,

    num_rows: usize,
    idx_row: usize,

    position: usize,
}


impl<'a> RowBytesIt<'a> {
    fn new<R>(
        reader: &'a mut Cursor<R>,
        ctx: &BinTable,
    ) -> Self
    where
        R: AsRef<[u8]> + 'a,
    {
        let num_bytes_in_row = ctx.naxis1 as usize;
        let num_rows = ctx.naxis2 as usize;
        let idx_row = 0;

        let r = reader.get_ref();
        let bytes = r.as_ref();
        let position = reader.position() as usize;

        Self {
            bytes,
            position,
            num_rows,
            idx_row,
            num_bytes_in_row,
        }
    }

    /// Returns the byte offset index in the cursor where data begins
    pub fn position(&self) -> u64 {
        self.position as u64
    }
}


impl<'a> Iterator for RowBytesIt<'a> {
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx_row == self.num_rows {
            None
        } else {
            // The cursor is always positioned at the beginning of the main table data.
            // i.e. nothing is read until all the data block has been read (with the heap)
            // once it is done the consume_until_hdu method will perform the read
            let start_table_pos = self.position;
            let start_row_off = start_table_pos + self.idx_row * self.num_bytes_in_row;
            let end_row_off = start_row_off + self.num_bytes_in_row;

            let row_bytes = &self.bytes[start_row_off..end_row_off];

            self.idx_row = self.idx_row + 1;

            Some(row_bytes)
        }
    }
}


#[derive(Debug)]
pub struct RowIt<'a> {
    /// A simple iterator over the bytes of a row
    row_bytes_it: RowBytesIt<'a>,

    /// Context of the binary table
    /// It contains all the mandatory and optional cards parsed from the header unit
    pub ctx: BinTable,
}

impl<'a> RowIt<'a> {
    pub fn new<R>(
        reader: &'a mut Cursor<R>,
        ctx: &BinTable,
    ) -> Self
    where
        R: AsRef<[u8]> + 'a,
    {
        let row_bytes_it = RowBytesIt::new(reader, ctx);

        Self {
            row_bytes_it,
            ctx: ctx.clone()
        }
    }

    pub fn bytes(self) -> RowBytesIt<'a> {
        self.row_bytes_it
    }
}

use super::Row;
impl<'a> Iterator for RowIt<'a> {
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = Row<'a, &'a [u8]>;

    fn next(&mut self) -> Option<Self::Item> {
        let row_bytes = self.row_bytes_it.next()?;

        let off_bytes_in_row = 0;
        let row = self.ctx.tforms.iter().map(|tform| {
            let start_off_byte = off_bytes_in_row;
            let end_off_byte = start_off_byte + tform.num_bytes_field();

            let field_bytes = &row_bytes[start_off_byte..end_off_byte];

            match tform {
                TFormType::L { .. } => FieldTy::Logical(
                    field_bytes
                        .iter()
                        .map(|v| *v != 0)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormType::B { .. } => FieldTy::UnsignedByte(field_bytes.into()),
                TFormType::A { .. } => FieldTy::Character(field_bytes.into()),
                TFormType::X { repeat_count } => FieldTy::Bit {
                    bytes: field_bytes.into(),
                    num_bits: *repeat_count,
                },
                TFormType::I { .. } => FieldTy::Short(
                    field_bytes
                        .chunks(2)
                        .map(|v| BigEndian::read_i16(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormType::J { .. } => FieldTy::Integer(
                    field_bytes
                        .chunks(4)
                        .map(|v| BigEndian::read_i32(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormType::K { .. } => FieldTy::Long(
                    field_bytes
                        .chunks(8)
                        .map(|v| BigEndian::read_i64(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormType::E { .. } => FieldTy::Float(
                    field_bytes
                        .chunks(4)
                        .map(|v| BigEndian::read_f32(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormType::D { .. } => FieldTy::Double(
                    field_bytes
                        .chunks(8)
                        .map(|v| BigEndian::read_f64(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormType::C { .. } => FieldTy::ComplexFloat(Box::new([])),
                TFormType::M { .. } => FieldTy::ComplexDouble(Box::new([])),
                TFormType::P { ty, t_byte_size, .. } => {
                    let (n_elems, byte_offset) = P::parse_array_location(field_bytes);

                    // seek to the heap location where the start of the array lies
                    let off =
                    // from the beginning of the main table go to the beginning of the heap
                        self.ctx.theap
                    // from the beginning of the heap go to the start of the array
                        + byte_offset as usize;

                    // as the reader is positioned at the beginning of the main data table
                    let num_bytes = (n_elems as usize) * (*t_byte_size as usize);
                    let start_array_off = self.row_bytes_it.position + off;
                    let end_array_off = start_array_off + num_bytes;
                    let array_raw_bytes = &self.row_bytes_it.bytes[start_array_off..end_array_off];

                    FieldTy::parse_variable_array(array_raw_bytes, *ty, num_bytes as u64, &self.ctx)
                },
                TFormType::Q { ty, t_byte_size, .. } => {
                    let (n_elems, byte_offset) = Q::parse_array_location(field_bytes);

                    // seek to the heap location where the start of the array lies
                    let off =
                    // from the beginning of the main table go to the beginning of the heap
                        self.ctx.theap
                    // from the beginning of the heap go to the start of the array
                        + byte_offset as usize;

                    // as the reader is positioned at the beginning of the main data table
                    let num_bytes = (n_elems as usize) * (*t_byte_size as usize);
                    let start_array_off = self.row_bytes_it.position + off;
                    let end_array_off = start_array_off + num_bytes;
                    let array_raw_bytes = &self.row_bytes_it.bytes[start_array_off..end_array_off];

                    FieldTy::parse_variable_array(array_raw_bytes, *ty, num_bytes as u64, &self.ctx)
                },
            }
        }).collect::<Vec<_>>();

        Some(row)
    }
}

impl<'a, R> DataRead<'a, BinTable> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + 'a,
{
    type Data = RowIt<'a>;
    fn new(
        reader: &'a mut Self,
        ctx: &BinTable,
        _num_remaining_bytes_in_cur_hdu: &'a mut usize,
    ) -> Self::Data {
        RowIt::new(reader, ctx)
    }
}
*/