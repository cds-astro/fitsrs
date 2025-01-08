//pub use super::Access;
//use super::DataAsyncBufRead;
use crate::error::Error;
use crate::fits::Fits;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::BitpixValue;
use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder};
use futures::AsyncRead;
use std::cell::UnsafeCell;
use std::io::{BufReader, Cursor, Read};

use std::fmt::Debug;

use super::{iter, InMemoryData, Slice};
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::extension::Xtension;
use crate::hdu::DataBufRead;

impl<'a, R> DataBufRead<'a, Image> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + Read + 'a,
{
    type Data = Slice<'a>;

    fn prepare_data_reading(
        ctx: &Image,
        num_bytes_read: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        let num_bytes_of_data = ctx.get_num_bytes_data_block() as usize;
        // propagate the number of bytes read to the hdu_list iterator so that it knows where the reader is.
        *num_bytes_read += num_bytes_of_data;

        let bitpix = ctx.get_bitpix();

        let bytes = reader.get_ref();
        let bytes = bytes.as_ref();

        let pos = reader.position() as usize;

        let start_byte_pos = pos;
        let end_byte_pos = pos + num_bytes_of_data;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        let c = bytes as *const [u8] as *mut UnsafeCell<[u8]>;
        unsafe {
            let cell: &UnsafeCell<[u8]> = &*c;
            let x_mut_ref = &mut *cell.get();

            match bitpix {
                BitpixValue::U8 => {
                    let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
                    let num_pixels = num_bytes_of_data as usize;

                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Slice::U8(data)
                }
                BitpixValue::I16 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<i16>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_i16(data);

                    // 3. Keep only the pixels
                    let num_pixels = (num_bytes_of_data as usize) / std::mem::size_of::<i16>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Slice::I16(data)
                }
                BitpixValue::I32 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<i32>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_i32(data);

                    // 3. Keep only the pixels
                    let num_pixels = (num_bytes_of_data as usize) / std::mem::size_of::<i32>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Slice::I32(data)
                }
                BitpixValue::I64 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<i64>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_i64(data);
                    // 3. Keep only the pixels
                    let num_pixels = num_bytes_of_data / std::mem::size_of::<i64>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Slice::I64(data)
                }
                BitpixValue::F32 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<f32>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_f32(data);
                    // 3. Keep only the pixels
                    let num_pixels = num_bytes_of_data / std::mem::size_of::<f32>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Slice::F32(data)
                }
                BitpixValue::F64 => {
                    // 1. Verify the alignement
                    let (_, data, _) = x_mut_ref.align_to_mut::<f64>();
                    // 2. Convert to big endianness. This is O(N) over the size of the data
                    BigEndian::from_slice_f64(data);
                    // 3. Keep only the pixels
                    let num_pixels = num_bytes_of_data / std::mem::size_of::<f64>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    Slice::F64(data)
                }
            }
        }
    }

    /*fn consume_data_block(self, data: Self::Data, num_bytes_read: &mut u64) -> Result<Self, Error> {
        let Data {
            num_bytes_read: num_bytes,
            ..
        } = data;
        *num_bytes_read = num_bytes as u64;

        self.set_position(self.position() + num_bytes as u64);

        Ok(self)
    }*/
}

impl<'a, R> DataBufRead<'a, Image> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = iter::Iter<'a, Self>;

    fn prepare_data_reading(
        ctx: &Image,
        num_bytes_read: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        let num_bytes_to_read = ctx.get_num_bytes_data_block() as usize;
        let bitpix = ctx.get_bitpix();
        match bitpix {
            BitpixValue::U8 => {
                iter::Iter::U8(iter::It::new(reader, num_bytes_read, num_bytes_to_read))
            }
            BitpixValue::I16 => {
                iter::Iter::I16(iter::It::new(reader, num_bytes_read, num_bytes_to_read))
            }
            BitpixValue::I32 => {
                iter::Iter::I32(iter::It::new(reader, num_bytes_read, num_bytes_to_read))
            }
            BitpixValue::I64 => {
                iter::Iter::I64(iter::It::new(reader, num_bytes_read, num_bytes_to_read))
            }
            BitpixValue::F32 => {
                iter::Iter::F32(iter::It::new(reader, num_bytes_read, num_bytes_to_read))
            }
            BitpixValue::F64 => {
                iter::Iter::F64(iter::It::new(reader, num_bytes_read, num_bytes_to_read))
            }
        }
    }

    /*
    fn consume_data_block(
        &mut self,
        data: Self::Data,
        num_bytes_read: &mut u64,
    ) -> Result<&mut Self, Error> {
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
    }*/
}

/*
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
*/
