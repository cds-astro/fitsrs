use byteorder::ByteOrder;
use futures::AsyncReadExt;

/// An async iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Debug)]
pub enum DataStream<'a, R>
where
    R: AsyncBufRead + Unpin,
{
    U8(St<'a, R, u8>),
    I16(St<'a, R, i16>),
    I32(St<'a, R, i32>),
    I64(St<'a, R, i64>),
    F32(St<'a, R, f32>),
    F64(St<'a, R, f64>),
}

#[derive(Debug)]
pub struct St<'a, R, T>
where
    R: AsyncBufRead + Unpin,
{
    pub reader: &'a mut R,
    pub num_remaining_bytes_in_cur_hdu: &'a mut usize,
    phantom: std::marker::PhantomData<T>,
}

impl<'a, R, T> St<'a, R, T>
where
    R: AsyncBufRead + Unpin,
{
    pub fn new(reader: &'a mut R, num_remaining_bytes_in_cur_hdu: &'a mut usize) -> Self {
        Self {
            reader,
            num_remaining_bytes_in_cur_hdu,
            phantom: std::marker::PhantomData,
        }
    }
}

use futures::task::Context;
use futures::task::Poll;
use futures::AsyncBufRead;
use futures::Future;
//use serde::Serialize;
use std::pin::Pin;

//use super::Access;

impl<'a, R> futures::Stream for St<'a, R, u8>
where
    R: AsyncBufRead + Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[u8; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
            // The stream has finished
            Poll::Ready(None)
        } else {
            let mut buf = [0_u8; 1];

            let mut reader_exact = self.reader.read_exact(&mut buf);
            match Pin::new(&mut reader_exact).poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e))),
                Poll::Ready(Ok(())) => {
                    *self.num_remaining_bytes_in_cur_hdu -= 1;
                    Poll::Ready(Some(Ok(buf)))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for St<'a, R, i16>
where
    R: AsyncBufRead + Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[i16; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
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
                    *self.num_remaining_bytes_in_cur_hdu -= std::mem::size_of::<i16>();
                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for St<'a, R, i32>
where
    R: AsyncBufRead + Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[i32; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
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
                    *self.num_remaining_bytes_in_cur_hdu -= std::mem::size_of::<i32>();

                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for St<'a, R, i64>
where
    R: AsyncBufRead + Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[i64; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
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
                    *self.num_remaining_bytes_in_cur_hdu -= std::mem::size_of::<i64>();
                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for St<'a, R, f32>
where
    R: AsyncBufRead + Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[f32; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
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
                    *self.num_remaining_bytes_in_cur_hdu -= std::mem::size_of::<f32>();
                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

impl<'a, R> futures::Stream for St<'a, R, f64>
where
    R: AsyncBufRead + Unpin,
{
    /// The type of the value yielded by the stream.
    type Item = Result<[f64; 1], futures::io::Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
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
                    *self.num_remaining_bytes_in_cur_hdu -= std::mem::size_of::<f64>();
                    Poll::Ready(Some(Ok([item])))
                }
            }
        }
    }
}

/*
impl<'a, R> Access<'a> for St<'a, R, u8>
where
    R: AsyncBufRead + Unpin,
{
    type Type = &'a mut Self;

    fn get_data(&'a mut self) -> Self::Type {
        self
    }
}
*/
