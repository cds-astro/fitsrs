//pub use super::Access;
//use super::DataAsyncBufRead;

use crate::hdu::header::Bitpix;
use async_trait::async_trait;
use futures::AsyncReadExt;
use std::io::{BufReader, Read};

use super::super::{AsyncDataBufRead, DataIter, DataStream};
use crate::hdu::header::extension::image::Image;
use crate::hdu::DataRead;
use std::fmt::Debug;


impl<'a, R> DataRead<'a, Image> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = DataIter<'a, Self>;

    fn new(
        reader: &'a mut Self,
        ctx: &Image,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
    ) -> Self::Data {
        DataIter::new(ctx, num_remaining_bytes_in_cur_hdu, reader)
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
