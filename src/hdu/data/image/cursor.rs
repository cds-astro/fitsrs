use std::io::Cursor;
use std::fmt::Debug;
use std::borrow::Cow;

use crate::hdu::data::FitsRead;
use crate::hdu::data::iter::BigEndianIt;
use crate::hdu::header::{Bitpix, Xtension};
use crate::hdu::header::extension::image::Image;

use crate::hdu::data::Bytes;
/*#[derive(Debug)]
pub enum ImageData<'a> {
    U8(BigEndianIt<&'a [u8], u8>),
    I16(BigEndianIt<&'a [u8], i16>),
    I32(BigEndianIt<&'a [u8], i32>),
    I64(BigEndianIt<&'a [u8], i64>),
    F32(BigEndianIt<&'a [u8], f32>),
    F64(BigEndianIt<&'a [u8], f64>),
}

impl<'a, R> DataRead<'a, Image> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + 'a,
{
    type Data = ImageData<'a>;

    fn new(reader: &'a mut Self, ctx: &Image) -> Self::Data {
        let num_bytes_of_data = ctx.get_num_bytes_data_block() as usize;

        let bitpix = ctx.get_bitpix();

        let start_byte_pos = reader.position() as usize;

        let r = reader.get_ref();
        let bytes = r.as_ref();

        let end_byte_pos = start_byte_pos + num_bytes_of_data;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        let limit = bytes.len() as u64;

        match bitpix {
            Bitpix::U8 => {
                debug_assert!(bytes.len() >= num_bytes_of_data as usize);
                ImageData::U8(Cow::Borrowed(bytes))
            }
            Bitpix::I16 => ImageData::I16(BigEndianIt::new(bytes, limit)),
            Bitpix::I32 => ImageData::I32(BigEndianIt::new(bytes, limit)),
            Bitpix::I64 => ImageData::I64(BigEndianIt::new(bytes, limit)),
            Bitpix::F32 => ImageData::F32(BigEndianIt::new(bytes, limit)),
            Bitpix::F64 => ImageData::F64(BigEndianIt::new(bytes, limit)),
        }
    }
}*/