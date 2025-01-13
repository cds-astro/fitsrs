use std::fmt::Debug;

#[allow(unused_imports)]
use std::io::{BufReader, Cursor, Read};

#[allow(unused_imports)]
use async_trait::async_trait;
#[allow(unused_imports)]
use futures::AsyncReadExt;
#[allow(unused_imports)]
use serde::Serialize;

use crate::hdu::header::extension::bintable::{TFormBinaryTable, P};

use byteorder::{BigEndian, ByteOrder};

#[allow(unused_imports)]
use super::{iter, Data};
#[allow(unused_imports)]
use super::{stream, AsyncDataBufRead};
use crate::error::Error;
use crate::hdu::header::extension::bintable::{BinTable, TFormBinaryTableType};
use crate::hdu::DataBufRead;

use crate::hdu::header::extension::Xtension;

//use super::DataAsyncBufRead;
/*
impl<'a, R> DataBufRead<'a, BinTable> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + 'a,
{
    type Data = Data<'a>;

    fn prepare_data_reading(
        ctx: &BinTable,
        _num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        let num_bytes_of_data = ctx.get_num_bytes_data_block() as usize;

        let start_byte_pos = reader.position() as usize;

        let r = reader.get_ref();
        let bytes = r.as_ref();

        let end_byte_pos = start_byte_pos + num_bytes_of_data;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        let num_pixels = num_bytes_of_data as usize;

        debug_assert!(bytes.len() >= num_pixels);

        Data::U8(bytes)
    }
}
*/

#[derive(Debug)]
pub struct It<'a> {
    pub bytes: &'a [u8],
    //pub num_remaining_bytes_in_cur_hdu: &'a mut usize,
    num_bytes_in_table: usize,
    num_bytes_in_row: usize,

    pub tforms: Box<[TFormBinaryTableType]>,
    pub iform: usize,
    off_bytes_in_row: usize,
    pub theap: usize,
    idx_row: usize,
    position: usize,
}

impl<'a> It<'a> {
    pub fn new<R>(
        reader: &'a mut Cursor<R>,
        //num_remaining_bytes_in_cur_hdu: &'a mut usize,
        ctx: &BinTable,
    ) -> Self
    where
        R: AsRef<[u8]> + 'a,
    {
        let tforms = ctx.tforms.clone().into_boxed_slice();
        let num_bytes_in_row = ctx.naxis1 as usize;
        let num_bytes_in_table = ctx.get_num_bytes_data_block() as usize;
        let theap = ctx.theap;
        let idx_row = 0;

        let r = reader.get_ref();
        let bytes = r.as_ref();
        let position = reader.position() as usize;

        Self {
            bytes,
            position,
            //num_remaining_bytes_in_cur_hdu,
            num_bytes_in_table,
            idx_row,
            num_bytes_in_row,
            tforms,
            iform: 0,
            off_bytes_in_row: 0,
            theap,
        }
    }
}

pub enum FieldTy<'a> {
    // 'L' => Ok(TFormBinaryTableType::L(TFormBinaryTable::new(count))), // Logical
    Logical(Box<[bool]>),
    // 'X' => Ok(TFormBinaryTableType::X(TFormBinaryTable::new(count))), // Bit
    Bit { bytes: &'a [u8], num_bits: usize },
    // 'B' => Ok(TFormBinaryTableType::B(TFormBinaryTable::new(count))), // Unsigned Byte
    UnsignedByte(&'a [u8]),
    // 'I' => Ok(TFormBinaryTableType::I(TFormBinaryTable::new(count))), // 16-bit integer
    Short(Box<[i16]>),
    // 'J' => Ok(TFormBinaryTableType::J(TFormBinaryTable::new(count))), // 32-bit integer
    Integer(Box<[i32]>),
    // 'K' => Ok(TFormBinaryTableType::K(TFormBinaryTable::new(count))), // 64-bit integer
    Long(Box<[i64]>),
    // 'A' => Ok(TFormBinaryTableType::A(TFormBinaryTable::new(count))), // Character
    Character(&'a [u8]),
    // 'E' => Ok(TFormBinaryTableType::E(TFormBinaryTable::new(count))), // Single-precision floating point
    Float(Box<[f32]>),
    // 'D' => Ok(TFormBinaryTableType::D(TFormBinaryTable::new(count))), // Double-precision floating point
    Double(Box<[f64]>),
    // 'C' => Ok(TFormBinaryTableType::C(TFormBinaryTable::new(count))), // Single-precision complex
    ComplexFloat(Box<[(f32, f32)]>),
    // 'M' => Ok(TFormBinaryTableType::M(TFormBinaryTable::new(count))), // Double-precision complex
    ComplexDouble(Box<[(f64, f64)]>),
    // 'P' => Ok(TFormBinaryTableType::P(TFormBinaryTable::new(count))), // Array Descriptor (32-bit)
    Array32Desc(Data<'a>),
    // 'Q' => Ok(TFormBinaryTableType::Q(TFormBinaryTable::new(count))), // Array Descriptor (64-bit)
    Array64Desc(Data<'a>),
}

impl<'a> Iterator for It<'a> {
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = Result<FieldTy<'a>, Error>;

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

            // we get the buffer filled of the current row
            let tform = self.tforms[self.iform];

            // pass to the next field
            self.iform = (self.iform + 1) % self.tforms.len();

            let start_off_byte = self.off_bytes_in_row;
            let end_off_byte = start_off_byte + tform.num_bytes_field();

            self.off_bytes_in_row = end_off_byte % self.num_bytes_in_row;
            if self.off_bytes_in_row == 0 {
                // new row
                self.idx_row = self.idx_row + 1;
            }

            let field_bytes = &row_bytes[start_off_byte..end_off_byte];

            let item = match tform {
                TFormBinaryTableType::L(_) => Ok(FieldTy::Logical(
                    field_bytes
                        .iter()
                        .map(|v| *v != 0)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )),
                TFormBinaryTableType::B(_) => Ok(FieldTy::UnsignedByte(field_bytes)),
                TFormBinaryTableType::X(x) => Ok(FieldTy::Bit {
                    bytes: field_bytes,
                    num_bits: x.num_bits_field(),
                }),
                TFormBinaryTableType::I(_) => Ok(FieldTy::Short(
                    field_bytes
                        .chunks(2)
                        .map(|v| BigEndian::read_i16(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )),
                TFormBinaryTableType::J(_) => Ok(FieldTy::Integer(
                    field_bytes
                        .chunks(4)
                        .map(|v| BigEndian::read_i32(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )),
                TFormBinaryTableType::K(_) => Ok(FieldTy::Long(
                    field_bytes
                        .chunks(8)
                        .map(|v| BigEndian::read_i64(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )),
                TFormBinaryTableType::A(_) => Ok(FieldTy::Character(field_bytes)),
                TFormBinaryTableType::E(_) => Ok(FieldTy::Float(
                    field_bytes
                        .chunks(4)
                        .map(|v| BigEndian::read_f32(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )),
                TFormBinaryTableType::D(_) => Ok(FieldTy::Double(
                    field_bytes
                        .chunks(8)
                        .map(|v| BigEndian::read_f64(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )),
                TFormBinaryTableType::C(_) => Ok(FieldTy::ComplexFloat(Box::new([]))),
                TFormBinaryTableType::M(_) => Ok(FieldTy::ComplexDouble(Box::new([]))),
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
                    let start_array_off = self.position + off;
                    let end_array_off = start_array_off + (n_elems as usize) * config.t_byte_size;
                    let array_raw_bytes = &self.bytes[start_array_off..end_array_off];

                    // once we have the bytes, convert it accordingly
                    match config.ty {
                        'B' => Ok(Data::U8(array_raw_bytes)),
                        'I' => Ok(Data::I16(
                            array_raw_bytes
                                .chunks(2)
                                .map(|item| BigEndian::read_i16(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        )),
                        'J' => Ok(Data::I32(
                            array_raw_bytes
                                .chunks(4)
                                .map(|item| BigEndian::read_i32(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        )),
                        'K' => Ok(Data::I64(
                            array_raw_bytes
                                .chunks(8)
                                .map(|item| BigEndian::read_i64(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        )),
                        'E' => Ok(Data::F32(
                            array_raw_bytes
                                .chunks(4)
                                .map(|item| BigEndian::read_f32(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        )),
                        'D' => Ok(Data::F64(
                            array_raw_bytes
                                .chunks(8)
                                .map(|item| BigEndian::read_f64(item))
                                .collect::<Vec<_>>()
                                .into_boxed_slice(),
                        )),
                        _ => Err(Error::StaticError(
                            "Type not supported for elements in an array",
                        )),
                    }
                    .map(|d| FieldTy::Array32Desc(d))
                }
                TFormBinaryTableType::Q(_) => {
                    unimplemented!()
                }
            };

            Some(item)
        }
    }
}

impl<'a, R> DataBufRead<'a, BinTable> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + Read + 'a,
{
    type Data = It<'a>;

    fn prepare_data_reading(
        ctx: &BinTable,
        _num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        It::new(reader, ctx)
    }
}

/*
#[async_trait(?Send)]
impl<'a, R> AsyncDataBufRead<'a, BinTable> for futures::io::BufReader<R>
where
    R: AsyncReadExt + Debug + 'a + std::marker::Unpin,
{
    type Data = stream::St<'a, Self, u8>;

    fn prepare_data_reading(
        _ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        stream::St::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}
*/
