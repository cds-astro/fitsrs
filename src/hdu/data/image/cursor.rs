use std::io::Cursor;
use std::fmt::Debug;

use crate::hdu::data::DataRead;
use crate::hdu::data::bintable::BigEndianIt;
use crate::hdu::header::{Bitpix, Xtension};
use crate::hdu::header::extension::image::Image;
use crate::hdu::data::iter::Data;

impl<'a, R> DataRead<'a, Image> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + 'a,
{
    type Data = Data<'a>;

    fn new(reader: &'a mut Self, ctx: &Image, _num_remaining_bytes_in_cur_hdu: &'a mut usize) -> Self::Data {
        let num_bytes_of_data = ctx.get_num_bytes_data_block() as usize;

        let bitpix = ctx.get_bitpix();

        let start_byte_pos = reader.position() as usize;

        let r = reader.get_ref();
        let bytes = r.as_ref();

        let end_byte_pos = start_byte_pos + num_bytes_of_data;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        match bitpix {
            Bitpix::U8 => {
                debug_assert!(bytes.len() >= num_bytes_of_data as usize);
                Data::U8(bytes)
            }
            Bitpix::I16 => Data::I16(BigEndianIt::new(bytes)),
            Bitpix::I32 => Data::I32(BigEndianIt::new(bytes)),
            Bitpix::I64 => Data::I64(BigEndianIt::new(bytes)),
            Bitpix::F32 => Data::F32(BigEndianIt::new(bytes)),
            Bitpix::F64 => Data::F64(BigEndianIt::new(bytes)),
        }
    }
}