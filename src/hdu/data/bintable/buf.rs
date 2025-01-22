use std::fmt::Debug;
use crate::hdu::data::{AsyncDataBufRead, stream::St};
use crate::hdu::header::Bitpix;
use crate::hdu::header::extension::bintable::{P, Q, ArrayDescriptor};
#[cfg(feature="tile-compressed-image")]
use crate::hdu::header::extension::bintable::ZCmpType;
use byteorder::{BigEndian, ByteOrder};
use crate::error::Error;
use crate::hdu::header::extension::bintable::{BinTable, TFormBinaryTableType};
use crate::hdu::DataRead;
use crate::hdu::header::extension::Xtension;

use std::io::{BufReader, Cursor};
use std::io::Read;
use super::{FieldTy, VariableArray, BigEndianIt, EitherIt};
use std::borrow::Cow;

impl<'a, R> DataRead<'a, BinTable> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = RowIt<'a, Self>;

    fn new(
        reader: &'a mut Self,
        ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
    ) -> Self::Data {
        RowIt::new(reader, ctx, num_remaining_bytes_in_cur_hdu)
    }
}

#[derive(Debug)]
pub struct RowBytesIt<'a, R> {
    /// The reader over the binary table data
    pub reader: &'a mut R,
    num_remaining_bytes_in_cur_hdu: &'a mut usize,
    num_bytes_in_row: usize,

    num_rows: usize,
    idx_row: usize,

    /// A buffer storing the current row bytes
    row_buf: Box<[u8]>,
}


impl<'a, R> RowBytesIt<'a, R> {
    fn new(
        reader: &'a mut R,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        ctx: &BinTable,
    ) -> Self {
        let num_bytes_in_row = ctx.naxis1 as usize;

        let num_rows: usize = ctx.get_num_bytes_data_block() as usize / num_bytes_in_row;
        let idx_row = 0;
        let row_buf = vec![0_u8; num_bytes_in_row].into_boxed_slice();

        Self {
            reader,
            num_remaining_bytes_in_cur_hdu,
            num_bytes_in_row,
            idx_row,
            num_rows,
            row_buf
        }
    }
}

impl<'a, R> Iterator for RowBytesIt<'a, R>
where
    R: Read
{
    type Item = Box<[u8]>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx_row == self.num_rows {
            None
        } else {
            // The cursor is always positioned at the beginning of the main table data.
            // i.e. nothing is read until all the data block has been read (with the heap)
            // once it is done the consume_until_hdu method will perform the read
            self.reader.read_exact(&mut self.row_buf).ok()?;

            self.idx_row = self.idx_row + 1;
            *self.num_remaining_bytes_in_cur_hdu -= self.num_bytes_in_row;

            Some(self.row_buf.clone())
        }
    }
}



#[derive(Debug)]
pub struct RowIt<'a, R> {
    row_bytes_it: RowBytesIt<'a, R>,

    /// Context of the binary table
    /// It contains all the mandatory and optional cards parsed from the header unit
    pub ctx: BinTable,
}

impl<'a, R> RowIt<'a, R> {
    pub fn new(
        reader: &'a mut R,
        ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
    ) -> Self {

        let row_bytes_it = RowBytesIt::new(reader, num_remaining_bytes_in_cur_hdu, ctx);
        Self {
            row_bytes_it,
            ctx: ctx.clone()
        }
    }

    pub fn bytes(self) -> RowBytesIt<'a, R> {
        self.row_bytes_it
    }
}

use super::Row;
use std::io::Seek;
impl<'a, R> Iterator for RowIt<'a, R>
where
    R: Read + Seek + 'a,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = Row<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let row_bytes = self.row_bytes_it.next()?;

        let mut fields = vec![];
        let mut off_bytes_in_row = 0;
        for tform in &self.ctx.tforms {
            let end_off_byte = off_bytes_in_row + tform.num_bytes_field();
            off_bytes_in_row = end_off_byte % (self.ctx.naxis1 as usize);

            let field_bytes = &row_bytes[off_bytes_in_row..end_off_byte];

            let field = match tform {
                TFormBinaryTableType::L { .. } => FieldTy::Logical(
                    field_bytes
                        .iter()
                        .map(|v| *v != 0)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::B { .. } => {
                    FieldTy::UnsignedByte(field_bytes.to_owned().into())
                }
                TFormBinaryTableType::A { .. } => {
                    FieldTy::Character(field_bytes.to_owned().into())
                }
                TFormBinaryTableType::X { repeat_count } => FieldTy::Bit {
                    bytes: field_bytes.to_owned().into(),
                    num_bits: *repeat_count,
                },
                TFormBinaryTableType::I { .. } => FieldTy::Short(
                    field_bytes
                        .chunks(2)
                        .map(|v| BigEndian::read_i16(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::J { .. } => FieldTy::Integer(
                    field_bytes
                        .chunks(4)
                        .map(|v| BigEndian::read_i32(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::K { .. } => FieldTy::Long(
                    field_bytes
                        .chunks(8)
                        .map(|v| BigEndian::read_i64(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::E { .. } => FieldTy::Float(
                    field_bytes
                        .chunks(4)
                        .map(|v| BigEndian::read_f32(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::D { .. } => FieldTy::Double(
                    field_bytes
                        .chunks(8)
                        .map(|v| BigEndian::read_f64(v))
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::P { ty, t_byte_size, .. } => {
                    let (n_elems, byte_offset) = P::parse_array_location(field_bytes);

                    // seek to the heap location where the start of the array lies
                    let off =
                        // go back to the beginning of the main table data block
                        - (self.ctx.get_num_bytes_data_block() as i64 - (*self.row_bytes_it.num_remaining_bytes_in_cur_hdu as i64))
                        // from the beginning of the main table go to the beginning of the heap
                        + self.ctx.theap as i64
                        // from the beginning of the heap go to the start of the array
                        + byte_offset as i64;

                    // as the reader is positioned at the beginning of the main data table
                    // go to variable length 0th item location
                    let _ = self.row_bytes_it.reader.seek_relative(off);
                    let num_bytes = (*t_byte_size as usize) * (n_elems as usize);
                    let mut array_raw_bytes = vec![0_u8; num_bytes];
                    self.row_bytes_it.reader.read_exact(&mut array_raw_bytes).ok()?;
                    // go back to the row
                    let _ = self.row_bytes_it.reader.seek_relative(-off - (num_bytes as i64));

                    FieldTy::parse_variable_array(array_raw_bytes.into(), *ty, &self.ctx)
                },

                TFormBinaryTableType::Q { ty, t_byte_size, .. } => {
                    let (n_elems, byte_offset) = Q::parse_array_location(field_bytes);

                    // seek to the heap location where the start of the array lies
                    let off =
                        // go back to the beginning of the main table data block
                        - (self.ctx.get_num_bytes_data_block() as i64 - (*self.row_bytes_it.num_remaining_bytes_in_cur_hdu as i64))
                        // from the beginning of the main table go to the beginning of the heap
                        + self.ctx.theap as i64
                        // from the beginning of the heap go to the start of the array
                        + byte_offset as i64;

                    // as the reader is positioned at the beginning of the main data table
                    // go to variable length 0th item location
                    let _ = self.row_bytes_it.reader.seek_relative(off);
                    let num_bytes = (*t_byte_size as usize) * (n_elems as usize);
                    let mut array_raw_bytes = vec![0_u8; num_bytes];
                    self.row_bytes_it.reader.read_exact(&mut array_raw_bytes).ok()?;
                    // go back to the row
                    let _ = self.row_bytes_it.reader.seek_relative(-off - (num_bytes as i64));

                    FieldTy::parse_variable_array(array_raw_bytes.into(), *ty, &self.ctx)
                },
                _ => FieldTy::UnsignedByte(field_bytes.to_owned().into())
            };

            fields.push(field);
        };

        Some(fields)
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
