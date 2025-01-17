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
use std::borrow::Cow;

use byteorder::{BigEndian, ByteOrder};

#[allow(unused_imports)]
use super::{iter, Data};
#[allow(unused_imports)]
use super::{stream, AsyncDataBufRead};
use crate::error::Error;
use crate::hdu::header::extension::bintable::{BinTable, TFormBinaryTableType};
use crate::hdu::DataRead;

use crate::hdu::header::extension::Xtension;

pub enum FieldTy<'a> {
    // 'L' => Logical
    Logical(Box<[bool]>),
    // 'X' => Bit
    Bit {
        bytes: Cow<'a, [u8]>,
        num_bits: usize,
    },
    // 'B' => Unsigned Byte
    UnsignedByte(Cow<'a, [u8]>),
    // 'I' => 16-bit integer
    Short(Box<[i16]>),
    // 'J' => 32-bit integer
    Integer(Box<[i32]>),
    // 'K' => 64-bit integer
    Long(Box<[i64]>),
    // 'A' => Character
    Character(Cow<'a, [u8]>),
    // 'E' => Single-precision floating point
    Float(Box<[f32]>),
    // 'D' => Double-precision floating point
    Double(Box<[f64]>),
    // 'C' => Single-precision complex
    ComplexFloat(Box<[(f32, f32)]>),
    // 'M' => Double-precision complex
    ComplexDouble(Box<[(f64, f64)]>),
    // 'P' => Array Descriptor (32-bit)
    Array32Desc(Data<'a>),
    // 'Q' => Array Descriptor (64-bit)
    Array64Desc(Data<'a>),
}

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
                TFormBinaryTableType::B(_) => Ok(FieldTy::UnsignedByte(Cow::Borrowed(field_bytes))),
                TFormBinaryTableType::A(_) => Ok(FieldTy::Character(Cow::Borrowed(field_bytes))),
                TFormBinaryTableType::X(x) => Ok(FieldTy::Bit {
                    bytes: Cow::Borrowed(field_bytes),
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
                        'B' => Ok(Data::U8(Cow::Borrowed(array_raw_bytes))),
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

impl<'a, R> DataRead<'a, BinTable> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + Read + 'a,
{
    type Data = It<'a>;

    fn init_data_reading_process(
        ctx: &BinTable,
        _num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        It::new(reader, ctx)
    }
}

impl<'a, R> DataRead<'a, BinTable> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = RowIt<'a, Self>;

    fn init_data_reading_process(
        ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        RowIt::new(reader, ctx, num_remaining_bytes_in_cur_hdu)
    }
}

#[derive(Debug)]
pub struct RowIt<'a, R> {
    pub reader: &'a mut R,
    pub num_remaining_bytes_in_cur_hdu: &'a mut usize,
    num_bytes_in_table: usize,
    num_bytes_in_row: usize,

    pub tforms: Box<[TFormBinaryTableType]>,
    row_buf: Box<[u8]>,
    pub theap: usize,
    idx_row: usize,
}

impl<'a, R> RowIt<'a, R> {
    pub fn new(
        reader: &'a mut R,
        ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
    ) -> Self {
        let tforms = ctx.tforms.clone().into_boxed_slice();
        let num_bytes_in_row = ctx.naxis1 as usize;
        let num_bytes_in_table = ctx.get_num_bytes_data_block() as usize;
        let theap = ctx.theap;
        let idx_row = 0;

        let row_buf = vec![0_u8; num_bytes_in_row].into_boxed_slice();

        Self {
            reader,
            num_remaining_bytes_in_cur_hdu,
            num_bytes_in_table,
            idx_row,
            num_bytes_in_row,
            tforms,
            row_buf,
            theap,
        }
    }
}

pub type Row<'a> = Box<[FieldTy<'a>]>;

use std::io::Seek;
impl<'a, R> Iterator for RowIt<'a, R>
where
    R: Read + Seek + 'a,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = Result<Row<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx_row * self.num_bytes_in_row == self.num_bytes_in_table {
            None
        } else {
            // read a row
            match self.reader.read_exact(&mut self.row_buf) {
                Err(e) => {
                    return Some(Err(Error::Io(e)));
                }
                _ => (),
            }

            *self.num_remaining_bytes_in_cur_hdu -= self.num_bytes_in_row;

            let row_bytes = &self.row_buf[..];

            let mut fields = vec![];
            let mut off_bytes_in_row = 0;
            for iform in 0..self.tforms.len() {
                // we get the buffer filled of the current row
                let tform = self.tforms[iform];

                let start_off_byte = off_bytes_in_row;
                let end_off_byte = start_off_byte + tform.num_bytes_field();

                off_bytes_in_row = end_off_byte % self.num_bytes_in_row;

                let field_bytes = &row_bytes[start_off_byte..end_off_byte];

                let field = match tform {
                    TFormBinaryTableType::L(_) => Ok(FieldTy::Logical(
                        field_bytes
                            .iter()
                            .map(|v| *v != 0)
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                    )),
                    TFormBinaryTableType::B(_) => {
                        Ok(FieldTy::UnsignedByte(Cow::Owned(field_bytes.to_vec())))
                    }
                    TFormBinaryTableType::A(_) => {
                        Ok(FieldTy::Character(Cow::Owned(field_bytes.to_vec())))
                    }
                    TFormBinaryTableType::X(x) => Ok(FieldTy::Bit {
                        bytes: Cow::Owned(field_bytes.to_vec()),
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
                        let off =
                        // go back to the beginning of the main table data block
                        - (self.num_bytes_in_table as i64 - (*self.num_remaining_bytes_in_cur_hdu as i64))
                        // from the beginning of the main table go to the beginning of the heap
                        + self.theap as i64
                        // from the beginning of the heap go to the start of the array
                        + byte_offset as i64;

                        // as the reader is positioned at the beginning of the main data table
                        // go to variable length 0th item location
                        let _ = self.reader.seek_relative(off);
                        let num_bytes = config.t_byte_size * (n_elems as usize);
                        let mut array_raw_bytes = vec![0_u8; num_bytes];
                        match self.reader.read_exact(&mut array_raw_bytes) {
                            Err(e) => return Some(Err(e.into())),
                            Ok(()) => {}
                        }
                        // go back to the row
                        let _ = self.reader.seek_relative(-off - (num_bytes as i64));
                        // once we have the bytes, convert it accordingly
                        match config.ty {
                            'B' => Ok(Data::U8(std::borrow::Cow::Owned(array_raw_bytes))),
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

                match field {
                    Err(e) => return Some(Err(e)),
                    Ok(field) => {
                        fields.push(field);
                    }
                }
            }
            self.idx_row = self.idx_row + 1;

            Some(Ok(fields.into_boxed_slice()))
        }
    }
}

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
