use crate::error::Error;
use async_trait::async_trait;
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use futures::AsyncRead;
use serde::Serialize;
use std::io::{BufRead, BufReader, Cursor, Read};

pub use super::Access;
use super::DataAsyncBufRead;
use crate::hdu::header::BitpixValue;

use std::fmt::Debug;

use crate::hdu::header::extension::image::Image;
use crate::hdu::DataBufRead;

use crate::hdu::header::extension::Xtension;

impl<'a, R> DataBufRead<'a, Image> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + Read + 'a,
{
    type Data = DataBorrowed<'a, Self>;

    fn new_data_block(&'a mut self, ctx: &Image) -> Self::Data
    where
        Self: Sized,
    {
        let num_bytes_read = ctx.get_num_bytes_data_block();

        let bitpix = ctx.get_bitpix();

        let bytes = self.get_ref();
        let bytes = bytes.as_ref();

        let pos = self.position() as usize;

        let start_byte_pos = pos;
        let end_byte_pos = pos + num_bytes_read;

        let bytes = &bytes[start_byte_pos..end_byte_pos];
        let x_ptr = bytes as *const [u8] as *mut [u8];
        unsafe {
            let x_mut_ref = &mut *x_ptr;

            match bitpix {
                BitpixValue::U8 => {
                    let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
                    let num_pixels = num_bytes_read;

                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    DataBorrowed {
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
                    let num_pixels = num_bytes_read / std::mem::size_of::<i16>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    DataBorrowed {
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
                    let num_pixels = num_bytes_read / std::mem::size_of::<i32>();
                    debug_assert!(data.len() >= num_pixels);
                    let data = &data[..num_pixels];

                    DataBorrowed {
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

                    DataBorrowed {
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

                    DataBorrowed {
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

                    DataBorrowed {
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
        num_bytes_read: &mut usize,
    ) -> Result<&'a mut Self, Error> {
        let DataBorrowed {
            reader,
            num_bytes_read: num_bytes,
            ..
        } = data;
        *num_bytes_read = num_bytes;

        reader.set_position(reader.position() + num_bytes as u64);

        Ok(reader)
    }
}

impl<'a, R> DataBufRead<'a, Image> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = DataOwned<'a, Self>;

    fn new_data_block(&'a mut self, ctx: &Image) -> Self::Data {
        let num_bytes_to_read = ctx.get_num_bytes_data_block();
        let bitpix = ctx.get_bitpix();
        match bitpix {
            BitpixValue::U8 => DataOwned::U8(DataOwnedIt::new(self, num_bytes_to_read)),
            BitpixValue::I16 => DataOwned::I16(DataOwnedIt::new(self, num_bytes_to_read)),
            BitpixValue::I32 => DataOwned::I32(DataOwnedIt::new(self, num_bytes_to_read)),
            BitpixValue::I64 => DataOwned::I64(DataOwnedIt::new(self, num_bytes_to_read)),
            BitpixValue::F32 => DataOwned::F32(DataOwnedIt::new(self, num_bytes_to_read)),
            BitpixValue::F64 => DataOwned::F64(DataOwnedIt::new(self, num_bytes_to_read)),
        }
    }

    fn consume_data_block(
        data: Self::Data,
        num_bytes_read: &mut usize,
    ) -> Result<&'a mut Self, Error> {
        let (reader, num_bytes_already_read, num_bytes_to_read) = match data {
            DataOwned::U8(DataOwnedIt {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            DataOwned::I16(DataOwnedIt {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            DataOwned::I32(DataOwnedIt {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            DataOwned::I64(DataOwnedIt {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            DataOwned::F32(DataOwnedIt {
                reader,
                num_bytes_read: num_bytes_already_read,
                num_bytes_to_read,
                ..
            }) => (reader, num_bytes_already_read, num_bytes_to_read),
            DataOwned::F64(DataOwnedIt {
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

/// The full slice of data found in-memory
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataBorrowed` structure
/// for in-memory readers (typically for `&[u8]` or a `Cursor<AsRef<[u8]>>`) that ensures
/// all the data fits in memory
///
#[derive(Serialize, Debug)]
pub struct DataBorrowed<'a, R>
where
    R: Read + Debug + 'a,
{
    pub reader: &'a mut R,
    pub num_bytes_read: usize,
    pub data: InMemData<'a>,
}

impl<'a, R> Access for DataBorrowed<'a, R>
where
    R: Read + Debug + 'a,
{
    type Type = InMemData<'a>;

    fn get_data(&self) -> &Self::Type {
        &self.data
    }

    fn get_data_mut(&mut self) -> &mut Self::Type {
        &mut self.data
    }
}

#[derive(Serialize, Debug, Clone)]
pub enum InMemData<'a> {
    U8(&'a [u8]),
    I16(&'a [i16]),
    I32(&'a [i32]),
    I64(&'a [i64]),
    F32(&'a [f32]),
    F64(&'a [f64]),
}

/// An iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Serialize, Debug)]
pub enum DataOwned<'a, R>
where
    R: BufRead,
{
    U8(DataOwnedIt<'a, R, u8>),
    I16(DataOwnedIt<'a, R, i16>),
    I32(DataOwnedIt<'a, R, i32>),
    I64(DataOwnedIt<'a, R, i64>),
    F32(DataOwnedIt<'a, R, f32>),
    F64(DataOwnedIt<'a, R, f64>),
}

impl<'a, R> Access for DataOwned<'a, R>
where
    R: BufRead,
{
    type Type = Self;

    fn get_data(&self) -> &Self::Type {
        self
    }

    fn get_data_mut(&mut self) -> &mut Self::Type {
        self
    }
}

#[derive(Serialize, Debug)]
pub struct DataOwnedIt<'a, R, T>
where
    R: BufRead,
{
    pub reader: &'a mut R,
    pub num_bytes_to_read: usize,
    pub num_bytes_read: usize,
    phantom: std::marker::PhantomData<T>,
}

impl<'a, R, T> DataOwnedIt<'a, R, T>
where
    R: BufRead,
{
    pub fn new(reader: &'a mut R, num_bytes_to_read: usize) -> Self {
        Self {
            reader,
            num_bytes_read: 0,
            num_bytes_to_read,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, u8>
where
    R: BufRead,
{
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_u8();
            self.num_bytes_read += std::mem::size_of::<Self::Item>();

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, i16>
where
    R: BufRead,
{
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_i16::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>();

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, i32>
where
    R: BufRead,
{
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_i32::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>();

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, i64>
where
    R: BufRead,
{
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_i64::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>();

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, f32>
where
    R: BufRead,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_f32::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>();

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, f64>
where
    R: BufRead,
{
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_f64::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>();

            item.ok()
        }
    }
}

#[async_trait]
impl<'a, R> DataAsyncBufRead<'a, Image> for futures::io::BufReader<R>
where
    R: AsyncRead + Debug + 'a + std::marker::Unpin + std::marker::Send,
{
    type Data = AsyncDataOwned<'a, Self>;

    fn new_data_block(&'a mut self, ctx: &Image) -> Self::Data {
        let num_bytes_to_read = ctx.get_num_bytes_data_block();
        let bitpix = ctx.get_bitpix();
        match bitpix {
            BitpixValue::U8 => AsyncDataOwned::U8(DataOwnedSt::new(self, num_bytes_to_read)),
            BitpixValue::I16 => AsyncDataOwned::I16(DataOwnedSt::new(self, num_bytes_to_read)),
            BitpixValue::I32 => AsyncDataOwned::I32(DataOwnedSt::new(self, num_bytes_to_read)),
            BitpixValue::I64 => AsyncDataOwned::I64(DataOwnedSt::new(self, num_bytes_to_read)),
            BitpixValue::F32 => AsyncDataOwned::F32(DataOwnedSt::new(self, num_bytes_to_read)),
            BitpixValue::F64 => AsyncDataOwned::F64(DataOwnedSt::new(self, num_bytes_to_read)),
        }
    }

    async fn consume_data_block(
        data: Self::Data,
        num_bytes_read: &mut usize,
    ) -> Result<&'a mut Self, Error>
    where
        'a: 'async_trait,
    {
        let (reader, num_bytes_to_read, num_bytes_already_read) = match data {
            AsyncDataOwned::U8(DataOwnedSt {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            AsyncDataOwned::I16(DataOwnedSt {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            AsyncDataOwned::I32(DataOwnedSt {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            AsyncDataOwned::I64(DataOwnedSt {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            AsyncDataOwned::F32(DataOwnedSt {
                reader,
                num_bytes_to_read,
                num_bytes_read,
                ..
            }) => (reader, num_bytes_to_read, num_bytes_read),
            AsyncDataOwned::F64(DataOwnedSt {
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

use futures::{AsyncBufRead, AsyncReadExt};

/// An async iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Serialize, Debug)]
pub enum AsyncDataOwned<'a, R>
where
    R: AsyncBufRead + std::marker::Unpin,
{
    U8(DataOwnedSt<'a, R, u8>),
    I16(DataOwnedSt<'a, R, i16>),
    I32(DataOwnedSt<'a, R, i32>),
    I64(DataOwnedSt<'a, R, i64>),
    F32(DataOwnedSt<'a, R, f32>),
    F64(DataOwnedSt<'a, R, f64>),
}

impl<'a, R> Access for AsyncDataOwned<'a, R>
where
    R: AsyncBufRead + std::marker::Unpin,
{
    type Type = Self;

    fn get_data(&self) -> &Self::Type {
        self
    }

    fn get_data_mut(&mut self) -> &mut Self::Type {
        self
    }
}

#[derive(Serialize, Debug)]
pub struct DataOwnedSt<'a, R, T>
where
    R: AsyncBufReadExt + std::marker::Unpin,
{
    pub reader: &'a mut R,
    pub num_bytes_to_read: usize,
    pub num_bytes_read: usize,
    phantom: std::marker::PhantomData<T>,
}

impl<'a, R, T> DataOwnedSt<'a, R, T>
where
    R: AsyncBufReadExt + std::marker::Unpin,
{
    pub fn new(reader: &'a mut R, num_bytes_to_read: usize) -> Self {
        let num_bytes_read = 0;
        Self {
            reader,
            num_bytes_read,
            num_bytes_to_read,
            phantom: std::marker::PhantomData,
        }
    }
}

use futures::task::Context;
use futures::task::Poll;
use futures::AsyncBufReadExt;
use futures::Future;
use futures::Stream;
use std::pin::Pin;

impl<'a, R> Stream for DataOwnedSt<'a, R, u8>
where
    R: AsyncBufReadExt + std::marker::Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[u8; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.num_bytes_read == self.num_bytes_to_read {
            // The stream has finished
            Poll::Ready(None)
        } else {
            let mut buf = [0_u8; 1];

            let mut reader_exact = self.reader.read_exact(&mut buf);
            match Pin::new(&mut reader_exact).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(())) => {
                    self.num_bytes_read += 1;
                    Poll::Ready(Some(Ok(buf)))
                }
            }
        }
    }
}

impl<'a, R> Stream for DataOwnedSt<'a, R, i16>
where
    R: AsyncBufReadExt + std::marker::Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[i16; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.num_bytes_to_read == self.num_bytes_read {
            // The stream has finished
            Poll::Ready(None)
        } else {
            let mut buf = [0_u8; 2];
            let mut reader_exact = self.reader.read_exact(&mut buf);
            match Pin::new(&mut reader_exact).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(())) => {
                    let item = byteorder::BigEndian::read_i16(&buf);
                    self.num_bytes_read += std::mem::size_of::<i16>();
                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for DataOwnedSt<'a, R, i32>
where
    R: AsyncBufReadExt + std::marker::Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[i32; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.num_bytes_to_read == self.num_bytes_read {
            // The stream has finished
            Poll::Ready(None)
        } else {
            let mut buf = [0_u8; 4];
            let mut reader_exact = self.reader.read_exact(&mut buf);
            match Pin::new(&mut reader_exact).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(())) => {
                    let item = byteorder::BigEndian::read_i32(&buf);
                    self.num_bytes_read += std::mem::size_of::<i32>();

                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for DataOwnedSt<'a, R, i64>
where
    R: AsyncBufReadExt + std::marker::Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[i64; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.num_bytes_to_read == self.num_bytes_read {
            // The stream has finished
            Poll::Ready(None)
        } else {
            let mut buf = [0_u8; 8];
            let mut reader_exact = self.reader.read_exact(&mut buf);
            match Pin::new(&mut reader_exact).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(())) => {
                    let item = byteorder::BigEndian::read_i64(&buf);
                    self.num_bytes_read += std::mem::size_of::<i64>();
                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for DataOwnedSt<'a, R, f32>
where
    R: AsyncBufReadExt + std::marker::Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[f32; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.num_bytes_to_read == self.num_bytes_read {
            // The stream has finished
            Poll::Ready(None)
        } else {
            let mut buf = [0_u8; 4];
            let mut reader_exact = self.reader.read_exact(&mut buf);
            match Pin::new(&mut reader_exact).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(())) => {
                    let item = byteorder::BigEndian::read_f32(&buf);
                    self.num_bytes_read += std::mem::size_of::<f32>();
                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for DataOwnedSt<'a, R, f64>
where
    R: AsyncBufReadExt + std::marker::Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[f64; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.num_bytes_read == self.num_bytes_to_read {
            // The stream has finished
            Poll::Ready(None)
        } else {
            let mut buf = [0_u8; 8];
            let mut reader_exact = self.reader.read_exact(&mut buf);
            match Pin::new(&mut reader_exact).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(())) => {
                    let item = byteorder::BigEndian::read_f64(&buf);
                    self.num_bytes_read += std::mem::size_of::<f64>();
                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> Access for DataOwnedSt<'a, R, u8>
where
    R: AsyncBufRead + std::marker::Unpin,
{
    type Type = Self;

    fn get_data(&self) -> &Self::Type {
        self
    }

    fn get_data_mut(&mut self) -> &mut Self::Type {
        self
    }
}
