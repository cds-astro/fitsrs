use std::fmt::Debug;
#[cfg(feature="tile-compressed-image")]
use crate::hdu::header::{
    Bitpix,
    extension::bintable::ZCmpType
};
use crate::hdu::header::extension::bintable::{TFormBinaryTable, P, Q};

use byteorder::{BigEndian, ByteOrder};
use flate2::read::GzDecoder;
use crate::hdu::header::extension::bintable::{BinTable, TFormBinaryTableType};
use crate::hdu::DataRead;
use crate::hdu::header::extension::Xtension;
use std::io::Cursor;

use super::{FieldTy, VariableArray, BigEndianIt, CastIt, EitherIt};

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
    type Item = Row<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let row_bytes = self.row_bytes_it.next()?;

        let off_bytes_in_row = 0;
        let row = self.ctx.tforms.iter().map(|tform| {
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
                    let off =
                    // from the beginning of the main table go to the beginning of the heap
                        self.ctx.theap
                    // from the beginning of the heap go to the start of the array
                        + byte_offset as usize;

                    // as the reader is positioned at the beginning of the main data table
                    let start_array_off = self.row_bytes_it.position + off;
                    let end_array_off = start_array_off + (n_elems as usize) * config.t_byte_size;
                    let array_raw_bytes = &self.row_bytes_it.bytes[start_array_off..end_array_off];

                    #[cfg(feature="tile-compressed-image")]
                    {
                        // TILE compressed convention case. More details here: https://fits.gsfc.nasa.gov/registry/tilecompression.html
                        if let Some(z_image) = &self.ctx.z_image {
                            let tile_raw_bytes = array_raw_bytes;

                            let field = match (z_image.z_cmp_type, z_image.z_bitpix) {
                                // It can only store integer typed values i.e. bytes, short or integers
                                (ZCmpType::Gzip1, Bitpix::U8) => VariableArray::U8(EitherIt::second(Cow::Borrowed(tile_raw_bytes).into())),
                                (ZCmpType::Gzip1, Bitpix::I16) => VariableArray::I16(EitherIt::second(Cow::Borrowed(tile_raw_bytes).into())),
                                (ZCmpType::Gzip1, Bitpix::I32) => VariableArray::I32(EitherIt::second(Cow::Borrowed(tile_raw_bytes).into())),
                                // Return the slice of bytes
                                _ => VariableArray::U8(EitherIt::first(Cow::Borrowed(tile_raw_bytes).into()))
                            };

                            return FieldTy::VariableArray(field);
                        }
                    }
                    // Once we have the bytes, convert it accordingly
                    let field = match config.ty {
                        'I' => VariableArray::I16(EitherIt::first(Cow::Borrowed(array_raw_bytes).into())),
                        'J' => VariableArray::I32(EitherIt::first(Cow::Borrowed(array_raw_bytes).into())),
                        'K' => VariableArray::I64(Cow::Borrowed(array_raw_bytes).into()),
                        'E' => VariableArray::F32(Cow::Borrowed(array_raw_bytes).into()),
                        'D' => VariableArray::F64(Cow::Borrowed(array_raw_bytes).into()),
                        _ => VariableArray::U8(EitherIt::first(Cow::Borrowed(array_raw_bytes).into())),
                    };

                    FieldTy::VariableArray(field)
                },
                TFormBinaryTableType::Q(TFormBinaryTable::<Q> { config, .. }) => todo!()
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
