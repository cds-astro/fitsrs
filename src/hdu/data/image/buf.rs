//pub use super::Access;
//use super::DataAsyncBufRead;

use crate::hdu::data::iter::BigEndianIt;
use crate::hdu::header::{Bitpix, Xtension};
use async_trait::async_trait;
use futures::AsyncReadExt;
use std::io::Read;
use serde::Serialize;

use super::super::{AsyncDataBufRead, DataStream};
use crate::hdu::header::extension::image::Image;
use crate::hdu::FitsRead;
use std::fmt::Debug;

impl<'a, R> FitsRead<'a, Image> for R
where
    R: Read + Debug + 'a,
{
    type Data = ImageData<&'a mut Self>;

    fn read_data_unit(&mut self,
        ctx: &Image,
    ) -> Self::Data {
        ImageData::new(ctx, self)
    }
}

use super::super::stream;
#[async_trait(?Send)]
impl<'a, R> AsyncDataBufRead<'a, Image> for futures::io::BufReader<R>
where
    R: AsyncReadExt + Debug + 'a + Unpin,
{
    type Data = DataStream<'a, Self>;

    fn prepare_data_reading(
        ctx: &Image,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        let bitpix = ctx.get_bitpix();
        match bitpix {
            Bitpix::U8 => {
                DataStream::U8(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            Bitpix::I16 => {
                DataStream::I16(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            Bitpix::I32 => {
                DataStream::I32(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            Bitpix::I64 => {
                DataStream::I64(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            Bitpix::F32 => {
                DataStream::F32(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            Bitpix::F64 => {
                DataStream::F64(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
        }
    }
}

/// An iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Serialize, Debug)]
pub enum ImageData<R> {
    U8(BigEndianIt<R, u8>),
    I16(BigEndianIt<R, i16>),
    I32(BigEndianIt<R, i32>),
    I64(BigEndianIt<R, i64>),
    F32(BigEndianIt<R, f32>),
    F64(BigEndianIt<R, f64>),
}

use std::io::Seek;
impl<R> ImageData<R>
where
    R: Read + Seek
{
    pub(crate) fn new(
        ctx: &Image,
        reader: R,
    ) -> Self {
        let limit = ctx.get_num_bytes_data_block();

        match ctx.get_bitpix() {
            Bitpix::U8 => ImageData::U8(BigEndianIt::new(reader, limit)),
            Bitpix::I16 => ImageData::I16(BigEndianIt::new(reader, limit)),
            Bitpix::I32 => ImageData::I32(BigEndianIt::new(reader, limit)),
            Bitpix::I64 => ImageData::I64(BigEndianIt::new(reader, limit)),
            Bitpix::F32 => ImageData::F32(BigEndianIt::new(reader, limit)),
            Bitpix::F64 => ImageData::F64(BigEndianIt::new(reader, limit)),
        }
    }
}