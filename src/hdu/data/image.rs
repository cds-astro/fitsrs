//pub use super::Access;
//use super::DataAsyncBufRead;

use crate::hdu::data::iter::It;
use crate::hdu::header::{Bitpix, Header, Xtension};
use async_trait::async_trait;
use futures::AsyncReadExt;
use serde::Serialize;
use std::io::Read;

use super::super::AsyncDataBufRead;
use super::DataStream;
use crate::hdu::header::extension::image::Image;
use crate::hdu::FitsRead;
use std::fmt::Debug;

impl<'a, R> FitsRead<'a, Image> for R
where
    R: Read + Debug + 'a,
{
    type Data = ImageData<&'a mut Self>;

    fn read_data_unit(&'a mut self, header: &Header<Image>, start_pos: u64) -> Self::Data {
        ImageData::new(header.get_xtension(), self, start_pos)
    }
}

#[derive(Serialize, Debug)]
pub struct ImageData<R> {
    start_pos: u64,
    num_bytes_data_block: u64,
    pixels: Pixels<R>,
}

/// An iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Serialize, Debug)]
pub enum Pixels<R> {
    U8(It<R, u8>),
    I16(It<R, i16>),
    I32(It<R, i32>),
    I64(It<R, i64>),
    F32(It<R, f32>),
    F64(It<R, f64>),
}

impl<R> ImageData<R>
where
    R: Read,
{
    pub(crate) fn new(ctx: &Image, reader: R, start_pos: u64) -> Self {
        let limit = ctx.get_num_bytes_data_block();

        let pixels = match ctx.get_bitpix() {
            Bitpix::U8 => Pixels::U8(It::new(reader, limit)),
            Bitpix::I16 => Pixels::I16(It::new(reader, limit)),
            Bitpix::I32 => Pixels::I32(It::new(reader, limit)),
            Bitpix::I64 => Pixels::I64(It::new(reader, limit)),
            Bitpix::F32 => Pixels::F32(It::new(reader, limit)),
            Bitpix::F64 => Pixels::F64(It::new(reader, limit)),
        };

        Self {
            start_pos,
            num_bytes_data_block: limit,
            pixels,
        }
    }

    /// Get the pixels iterator of the image
    pub fn pixels(self) -> Pixels<R> {
        self.pixels
    }
}

use std::io::Cursor;
impl<'a, R> ImageData<&'a mut Cursor<R>>
where
    R: AsRef<[u8]> + 'a,
{
    /// For in memory buffers, access the raw bytes of the image.
    /// You might need to convert the data from big to little endian at some point
    pub fn raw_bytes(self) -> &'a [u8] {
        let inner = match self.pixels {
            Pixels::U8(It { reader, .. })
            | Pixels::I16(It { reader, .. })
            | Pixels::I32(It { reader, .. })
            | Pixels::I64(It { reader, .. })
            | Pixels::F32(It { reader, .. })
            | Pixels::F64(It { reader, .. }) => reader.get_ref(),
        };
        let raw_bytes = inner.as_ref();

        let s = self.start_pos as usize;
        let e = s + (self.num_bytes_data_block as usize);
        &raw_bytes[s..e]
    }
}

use super::stream;
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
            Bitpix::U8 => DataStream::U8(stream::St::new(reader, num_remaining_bytes_in_cur_hdu)),
            Bitpix::I16 => DataStream::I16(stream::St::new(reader, num_remaining_bytes_in_cur_hdu)),
            Bitpix::I32 => DataStream::I32(stream::St::new(reader, num_remaining_bytes_in_cur_hdu)),
            Bitpix::I64 => DataStream::I64(stream::St::new(reader, num_remaining_bytes_in_cur_hdu)),
            Bitpix::F32 => DataStream::F32(stream::St::new(reader, num_remaining_bytes_in_cur_hdu)),
            Bitpix::F64 => DataStream::F64(stream::St::new(reader, num_remaining_bytes_in_cur_hdu)),
        }
    }
}
