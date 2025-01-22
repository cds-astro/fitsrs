use std::fmt::Debug;
use crate::hdu::data::{AsyncDataBufRead, stream::St};
use crate::hdu::header::Bitpix;
use crate::hdu::header::extension::bintable::{TFormBinaryTable, P};
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
                TFormBinaryTableType::L(_) => FieldTy::Logical(
                    field_bytes
                        .iter()
                        .map(|v| *v != 0)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                ),
                TFormBinaryTableType::B(_) => {
                    FieldTy::UnsignedByte(Cow::Owned(field_bytes.to_vec()))
                }
                TFormBinaryTableType::A(_) => {
                    FieldTy::Character(Cow::Owned(field_bytes.to_vec()))
                }
                TFormBinaryTableType::X(x) => FieldTy::Bit {
                    bytes: Cow::Owned(field_bytes.to_vec()),
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
                TFormBinaryTableType::P(TFormBinaryTable::<P> { config, .. }) => {
                    // get the number of elements in the array
                    let n_elems = BigEndian::read_u32(&field_bytes[0..4]);
                    // byte offset starting from the beginning of the heap
                    let byte_offset = BigEndian::read_u32(&field_bytes[4..8]);

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
                    let num_bytes = config.t_byte_size * (n_elems as usize);
                    let mut array_raw_bytes = vec![0_u8; num_bytes];
                    self.row_bytes_it.reader.read_exact(&mut array_raw_bytes).ok()?;
                    // go back to the row
                    let _ = self.row_bytes_it.reader.seek_relative(-off - (num_bytes as i64));

                    #[cfg(feature="tile-compressed-image")]
                    {
                        // TILE compressed convention case. More details here: https://fits.gsfc.nasa.gov/registry/tilecompression.html
                        if let Some(z_image) = &self.ctx.z_image {
                            let tile_raw_bytes = array_raw_bytes;

                            let field = match (z_image.z_cmp_type, z_image.z_bitpix) {
                                // It can only store integer typed values i.e. bytes, short or integers
                                (ZCmpType::Gzip1, Bitpix::U8) => VariableArray::U8(EitherIt::second(Cow::Owned::<[u8]>(tile_raw_bytes).into())),
                                (ZCmpType::Gzip1, Bitpix::I16) => VariableArray::I16(EitherIt::second(Cow::Owned::<[u8]>(tile_raw_bytes).into())),
                                (ZCmpType::Gzip1, Bitpix::I32) => VariableArray::I32(EitherIt::second(Cow::Owned::<[u8]>(tile_raw_bytes).into())),
                                // Return the slice of bytes
                                _ => VariableArray::U8(EitherIt::first(Cow::Owned::<[u8]>(tile_raw_bytes).into()))
                            };

                            FieldTy::VariableArray(field)
                        } else {
                            // once we have the bytes, convert it accordingly
                            FieldTy::VariableArray(match config.ty {
                                'I' => VariableArray::I16(EitherIt::first(Cow::Owned::<[u8]>(array_raw_bytes).into())),
                                'J' => VariableArray::I32(EitherIt::first(Cow::Owned::<[u8]>(array_raw_bytes).into())),
                                'K' => VariableArray::I64(Cow::Owned::<[u8]>(array_raw_bytes).into()),
                                'E' => VariableArray::F32(Cow::Owned::<[u8]>(array_raw_bytes).into()),
                                'D' => VariableArray::F64(Cow::Owned::<[u8]>(array_raw_bytes).into()),
                                _ => VariableArray::U8(EitherIt::first(Cow::Owned::<[u8]>(array_raw_bytes).into())),
                            })
                        }
                    }
                    #[cfg(not(feature="tile-compressed-image"))]
                    {
                        // once we have the bytes, convert it accordingly
                        FieldTy::VariableArray(match config.ty {
                            'I' => VariableArray::I16(EitherIt::first(Cow::Owned::<[u8]>(array_raw_bytes).into())),
                            'J' => VariableArray::I32(EitherIt::first(Cow::Owned::<[u8]>(array_raw_bytes).into())),
                            'K' => VariableArray::I64(Cow::Owned::<[u8]>(array_raw_bytes).into()),
                            'E' => VariableArray::F32(Cow::Owned::<[u8]>(array_raw_bytes).into()),
                            'D' => VariableArray::F64(Cow::Owned::<[u8]>(array_raw_bytes).into()),
                            _ => VariableArray::U8(EitherIt::first(Cow::Owned::<[u8]>(array_raw_bytes).into())),
                        })
                    }
                },
                _ => FieldTy::UnsignedByte(Cow::Owned(field_bytes.to_vec()))
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
