//pub use super::Access;
//use super::DataAsyncBufRead;

use crate::hdu::header::BitpixValue;
use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use futures::AsyncReadExt;
use std::io::{BufReader, Cursor, Read};

use std::fmt::Debug;

use super::{iter, AsyncDataBufRead, Data, DataIter, DataStream};
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::extension::Xtension;
use crate::hdu::DataBufRead;

impl<'a, R> DataBufRead<'a, Image> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + 'a,
{
    type Data = Data<'a>;

    fn prepare_data_reading(
        ctx: &Image,
        _num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        let num_bytes_of_data = ctx.get_num_bytes_data_block() as usize;

        let bitpix = ctx.get_bitpix();

        let start_byte_pos = reader.position() as usize;

        let r = reader.get_ref();
        let bytes = r.as_ref();

        let end_byte_pos = start_byte_pos + num_bytes_of_data;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        match bitpix {
            BitpixValue::U8 => {
                let num_pixels = num_bytes_of_data as usize;

                debug_assert!(bytes.len() >= num_pixels);

                Data::U8(bytes)
            }
            BitpixValue::I16 => {
                let data = bytes
                    .chunks(2)
                    .map(|item| BigEndian::read_i16(item))
                    .collect::<Vec<_>>();

                debug_assert!(data.len() == num_bytes_of_data / std::mem::size_of::<i16>());

                Data::I16(data.into_boxed_slice())
            }
            BitpixValue::I32 => {
                let data = bytes
                    .chunks(4)
                    .map(|item| BigEndian::read_i32(item))
                    .collect::<Vec<_>>();

                debug_assert!(data.len() == num_bytes_of_data / std::mem::size_of::<i32>());

                Data::I32(data.into_boxed_slice())
            }
            BitpixValue::I64 => {
                let data = bytes
                    .chunks(8)
                    .map(|item| BigEndian::read_i64(item))
                    .collect::<Vec<_>>();

                debug_assert!(data.len() == num_bytes_of_data / std::mem::size_of::<i64>());

                Data::I64(data.into_boxed_slice())
            }
            BitpixValue::F32 => {
                let data = bytes
                    .chunks(4)
                    .map(|item| BigEndian::read_f32(item))
                    .collect::<Vec<_>>();

                debug_assert!(data.len() == num_bytes_of_data / std::mem::size_of::<f32>());

                Data::F32(data.into_boxed_slice())
            }
            BitpixValue::F64 => {
                let data = bytes
                    .chunks(8)
                    .map(|item| BigEndian::read_f64(item))
                    .collect::<Vec<_>>();

                debug_assert!(data.len() == num_bytes_of_data / std::mem::size_of::<f64>());

                Data::F64(data.into_boxed_slice())
            }
        }
    }
}

impl<'a, R> DataBufRead<'a, Image> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = DataIter<'a, Self>;

    fn prepare_data_reading(
        ctx: &Image,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        let bitpix = ctx.get_bitpix();
        match bitpix {
            BitpixValue::U8 => DataIter::U8(iter::It::new(reader, num_remaining_bytes_in_cur_hdu)),
            BitpixValue::I16 => {
                DataIter::I16(iter::It::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::I32 => {
                DataIter::I32(iter::It::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::I64 => {
                DataIter::I64(iter::It::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::F32 => {
                DataIter::F32(iter::It::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::F64 => {
                DataIter::F64(iter::It::new(reader, num_remaining_bytes_in_cur_hdu))
            }
        }
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
            BitpixValue::U8 => {
                DataStream::U8(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::I16 => {
                DataStream::I16(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::I32 => {
                DataStream::I32(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::I64 => {
                DataStream::I64(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::F32 => {
                DataStream::F32(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
            BitpixValue::F64 => {
                DataStream::F64(stream::St::new(reader, num_remaining_bytes_in_cur_hdu))
            }
        }
    }
}
