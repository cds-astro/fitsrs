use std::fmt::Debug;
use crate::hdu::data::Data;
use crate::hdu::header::extension::bintable::{TFormBinaryTable, P};
use byteorder::{BigEndian, ByteOrder};
use crate::hdu::header::extension::bintable::{BinTable, TFormBinaryTableType};
use crate::hdu::DataRead;
use crate::hdu::header::extension::Xtension;
use std::io::Cursor;

use super::FieldTy;

use std::borrow::Cow;

#[derive(Debug)]
pub struct RowBytesIt<'a> {
    // The bytes over the whole data block
    pub bytes: &'a [u8],

    num_bytes_in_table: usize,
    num_bytes_in_row: usize,

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
        let num_bytes_in_table: usize = ctx.get_num_bytes_data_block() as usize;
        let idx_row = 0;

        let r = reader.get_ref();
        let bytes = r.as_ref();
        let position = reader.position() as usize;

        Self {
            bytes,
            position,
            num_bytes_in_table,
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
        if self.idx_row * self.num_bytes_in_row == self.num_bytes_in_table {
            None
        } else {
            // The cursor is always positioned at the beginning of the main table data.
            // i.e. nothing is read until all the data block has been read (with the heap)
            // once it is done the consume_until_hdu method will perform the read
            let start_table_pos = self.position as usize;
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
    row_bytes_it: RowBytesIt<'a>,

    pub tforms: Box<[TFormBinaryTableType]>,
    pub theap: usize,
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

        let tforms = ctx.tforms.clone().into_boxed_slice();
        let theap = ctx.theap;

        Self {
            row_bytes_it,
            tforms,
            theap,
        }
    }

    pub fn bytes(self) -> RowBytesIt<'a> {
        self.row_bytes_it
    }
}

use super::Row;
impl<'a> Iterator for RowIt<'a> {
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = Row<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let row_bytes = self.row_bytes_it.next()?;

        let off_bytes_in_row = 0;
        let row = self.tforms.iter().map(|tform| {
            let start_off_byte = off_bytes_in_row;
            let end_off_byte = start_off_byte + tform.num_bytes_field();

            let field_bytes = &row_bytes[start_off_byte..end_off_byte];

            match tform {
                TFormBinaryTableType::L(_) => FieldTy::Logical(
                    field_bytes
                        .iter()
                        .map(|v| *v != 0)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::B(_) => FieldTy::UnsignedByte(Cow::Borrowed(field_bytes)),
                TFormBinaryTableType::A(_) => FieldTy::Character(Cow::Borrowed(field_bytes)),
                TFormBinaryTableType::X(x) => FieldTy::Bit {
                    bytes: Cow::Borrowed(field_bytes),
                    num_bits: x.num_bits_field(),
                },
                TFormBinaryTableType::I(_) => FieldTy::Short(
                    field_bytes
                        .chunks(2)
                        .map(|v| BigEndian::read_i16(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::J(_) => FieldTy::Integer(
                    field_bytes
                        .chunks(4)
                        .map(|v| BigEndian::read_i32(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::K(_) => FieldTy::Long(
                    field_bytes
                        .chunks(8)
                        .map(|v| BigEndian::read_i64(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::E(_) => FieldTy::Float(
                    field_bytes
                        .chunks(4)
                        .map(|v| BigEndian::read_f32(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::D(_) => FieldTy::Double(
                    field_bytes
                        .chunks(8)
                        .map(|v| BigEndian::read_f64(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::C(_) => FieldTy::ComplexFloat(Box::new([])),
                TFormBinaryTableType::M(_) => FieldTy::ComplexDouble(Box::new([])),
                TFormBinaryTableType::P(TFormBinaryTable::<P> { config, .. }) => {
                    // get the number of elements in the array
                    let n_elems = BigEndian::read_u32(&field_bytes[0..4]);
                    // byte offset starting from the beginning of the heap
                    let byte_offset = BigEndian::read_u32(&field_bytes[4..8]);

                    // seek to the heap location where the start of the array lies
                    /*let off =
                    // go back to the beginning of the main table data block
                    - (self.num_bytes_in_table as i64 - (*self.num_remaining_bytes_in_cur_hdu as i64))
                    // from the beginning of the main table go to the beginning of the heap
                    + self.theap as i64
                    // from the beginning of the heap go to the start of the array
                    + byte_offset as i64;*/
                    let off =
                    // from the beginning of the main table go to the beginning of the heap
                        self.theap
                    // from the beginning of the heap go to the start of the array
                        + byte_offset as usize;

                    // as the reader is positioned at the beginning of the main data table
                    let start_array_off = self.row_bytes_it.position + off;
                    let end_array_off = start_array_off + (n_elems as usize) * config.t_byte_size;
                    let array_raw_bytes = &self.row_bytes_it.bytes[start_array_off..end_array_off];

                    // once we have the bytes, convert it accordingly
                    let field = match config.ty {
                        'B' => Data::U8(Cow::Borrowed(array_raw_bytes)),
                        'I' => Data::I16(
                            array_raw_bytes
                                .chunks(2)
                                .map(|item| BigEndian::read_i16(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        ),
                        'J' => Data::I32(
                            array_raw_bytes
                                .chunks(4)
                                .map(|item| BigEndian::read_i32(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        ),
                        'K' => Data::I64(
                            array_raw_bytes
                                .chunks(8)
                                .map(|item| BigEndian::read_i64(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        ),
                        'E' => Data::F32(
                            array_raw_bytes
                                .chunks(4)
                                .map(|item| BigEndian::read_f32(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        ),
                        'D' => Data::F64(
                            array_raw_bytes
                                .chunks(8)
                                .map(|item| BigEndian::read_f64(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        ),
                        _ => Data::U8(Cow::Borrowed(b"Value not parsed")),
                    };

                    FieldTy::Array32Desc(field)
                }
                TFormBinaryTableType::Q(_) => {
                    // TODO: same logic as the P case
                    FieldTy::Array64Desc(Data::U8(Cow::Owned(vec![])))
                }
            }
        }).collect::<Vec<_>>().into_boxed_slice();

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