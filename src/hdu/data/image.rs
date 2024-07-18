pub use super::Access;
use super::DataAsyncBufRead;
use crate::error::Error;
use crate::hdu::header::BitpixValue;
use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use futures::AsyncRead;
use std::cell::UnsafeCell;
use std::io::{BufReader, Cursor, Read};

use std::fmt::Debug;

use super::iter;
use super::{Data, InMemData};
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::extension::Xtension;
use crate::hdu::DataBufRead;

impl<'a, R> DataBufRead<'a, Image> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + Read + 'a,
{
    type Data = Data<'a, Self>;

    fn new_data_block(&'a mut self, ctx: &Image) -> Self::Data
    where
        Self: Sized,
    {
        let num_bytes_read = ctx.get_num_bytes_data_block() as usize;

        let bitpix = ctx.get_bitpix();

        let bytes = self.get_ref();
        let bytes = bytes.as_ref();

        let pos = self.position() as usize;

        let start_byte_pos = pos;
        let end_byte_pos = pos + num_bytes_read;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        let c = bytes as *const [u8] as *mut UnsafeCell<[u8]>;
        unsafe {
            let cell: &UnsafeCell<[u8]> = &*c;
            let x_mut_ref = &mut *cell.get();

            match bitpix {
                BitpixValue::U8 => {
                    let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
                    let num_pixels = num_bytes_read as usize;

                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Data {
                        data: InMemData::U8(data),
                        reader: self,
                        num_bytes_read,
                    }
                }
                BitpixValue::I16 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<i16>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_i16(data);

                    // 3. Keep only the pixels
                    let num_pixels = (num_bytes_read as usize) / std::mem::size_of::<i16>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Data {
                        data: InMemData::I16(data),
                        reader: self,
                        num_bytes_read,
                    }
                }
                BitpixValue::I32 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<i32>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_i32(data);

                    // 3. Keep only the pixels
                    let num_pixels = (num_bytes_read as usize) / std::mem::size_of::<i32>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Data {
                        data: InMemData::I32(data),
                        reader: self,
                        num_bytes_read,
                    }
                }
                BitpixValue::I64 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<i64>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_i64(data);
                    // 3. Keep only the pixels
                    let num_pixels = num_bytes_read / std::mem::size_of::<i64>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Data {
                        data: InMemData::I64(data),
                        reader: self,
                        num_bytes_read,
                    }
                }
                BitpixValue::F32 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<f32>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_f32(data);
                    // 3. Keep only the pixels
                    let num_pixels = num_bytes_read / std::mem::size_of::<f32>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Data {
                        data: InMemData::F32(data),
                        reader: self,
                        num_bytes_read,
                    }
                }
                BitpixValue::F64 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<f64>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_f64(data);
                    // 3. Keep only the pixels
                    let num_pixels = num_bytes_read / std::mem::size_of::<f64>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Data {
                        data: InMemData::F64(data),
                        reader: self,
                        num_bytes_read,
                    }
                }
            }
        }
    }

    fn consume_data_block(
        data: Self::Data,
        num_bytes_read: &mut u64,
    ) -> Result<&'a mut Self, Error> {
        let Data {
            reader,
            num_bytes_read: num_bytes,
            ..
        } = data;
        *num_bytes_read = num_bytes as u64;

        reader.set_position(reader.position() + num_bytes as u64);

        Ok(reader)
    }
}

impl<'a, R> DataBufRead<'a, Image> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = iter::Data<'a, Self>;

    fn new_data_block(&'a mut self, ctx: &Image) -> Self::Data {
        let num_bytes_to_read = ctx.get_num_bytes_data_block();
        let bitpix = ctx.get_bitpix();
        match bitpix {
            BitpixValue::U8 => iter::Data::U8(iter::Iter::new(self, num_bytes_to_read)),
            BitpixValue::I16 => iter::Data::I16(iter::Iter::new(self, num_bytes_to_read)),
            BitpixValue::I32 => iter::Data::I32(iter::Iter::new(self, num_bytes_to_read)),
            BitpixValue::I64 => iter::Data::I64(iter::Iter::new(self, num_bytes_to_read)),
            BitpixValue::F32 => iter::Data::F32(iter::Iter::new(self, num_bytes_to_read)),
            BitpixValue::F64 => iter::Data::F64(iter::Iter::new(self, num_bytes_to_read)),
        }
    }

    fn consume_data_block(
        data: Self::Data,
        num_bytes_read: &mut u64,
    ) -> Result<&'a mut Self, Error> {
        let (reader, num_bytes_already_read, num_bytes_to_read) = match data {
            iter::Data::U8(iter::Iter {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            iter::Data::I16(iter::Iter {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            iter::Data::I32(iter::Iter {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            iter::Data::I64(iter::Iter {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            iter::Data::F32(iter::Iter {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            iter::Data::F64(iter::Iter {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
        };

        let remaining_bytes_to_read = num_bytes_to_read - num_bytes_already_read;
        <Self as DataBufRead<'_, Image>>::read_n_bytes_exact(reader, remaining_bytes_to_read)?;

        // All the data block have been read
        *num_bytes_read = num_bytes_to_read;

        Ok(reader)
    }
}

use super::stream;
#[async_trait(?Send)]
impl<'a, R> DataAsyncBufRead<'a, Image> for futures::io::BufReader<R>
where
    R: AsyncRead + Debug + 'a + Unpin,
{
    type Data = super::stream::Data<'a, Self>;

    fn new_data_block(&'a mut self, ctx: &Image) -> Self::Data {
        let num_bytes_to_read = ctx.get_num_bytes_data_block();
        let bitpix = ctx.get_bitpix();
        match bitpix {
            BitpixValue::U8 => stream::Data::U8(stream::St::new(self, num_bytes_to_read)),
            BitpixValue::I16 => stream::Data::I16(stream::St::new(self, num_bytes_to_read)),
            BitpixValue::I32 => stream::Data::I32(stream::St::new(self, num_bytes_to_read)),
            BitpixValue::I64 => stream::Data::I64(stream::St::new(self, num_bytes_to_read)),
            BitpixValue::F32 => stream::Data::F32(stream::St::new(self, num_bytes_to_read)),
            BitpixValue::F64 => stream::Data::F64(stream::St::new(self, num_bytes_to_read)),
        }
    }

    async fn consume_data_block(
        data: Self::Data,
        num_bytes_read: &mut u64,
    ) -> Result<&'a mut Self, Error>
    where
        'a: 'async_trait,
    {
        let (reader, num_bytes_to_read, num_bytes_already_read) = match data {
            stream::Data::U8(stream::St {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            stream::Data::I16(stream::St {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            stream::Data::I32(stream::St {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            stream::Data::I64(stream::St {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            stream::Data::F32(stream::St {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            stream::Data::F64(stream::St {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
        };

        let remaining_bytes_to_read = num_bytes_to_read - num_bytes_already_read;
        <Self as DataAsyncBufRead<'_, Image>>::read_n_bytes_exact(reader, remaining_bytes_to_read)
            .await?;

        // All the data block have been read
        *num_bytes_read = num_bytes_to_read;

        Ok(reader)
    }
}
