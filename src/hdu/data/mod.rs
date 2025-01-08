pub mod asciitable;
pub mod bintable;
pub mod image;
pub mod iter;
pub mod layout;
pub mod stream;

use serde::Serialize;

use std::fmt::Debug;
use std::io::{BufRead, Read};

use std::marker::Unpin;

use crate::error::Error;
use crate::fits::Fits;
use crate::hdu::Xtension;

// reader must impl this
pub trait DataBufRead<'a, X>: BufRead
where
    X: Xtension,
{
    type Data: Debug + 'a;

    fn prepare_data_reading(
        ctx: &X,
        num_bytes_read: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data
    where
        Self: Sized;

    /// Consume the data to return back the reader at the position
    /// of the end of the data block
    ///
    /// If the data has not been fully read, we skip the remaining data
    /// bytes to go to the end of the data block
    ///
    /// # Params
    /// * `data` - a reader created i.e. from the opening of a file
    /*fn consume_data_block(
        &'a mut self,
        data: Self::Data,
        num_bytes_read: &mut u64,
    ) -> Result<&'a mut Self, Error>
    where
        Self: Sized;*/

    fn read_n_bytes_exact(&mut self, num_bytes_to_read: u64) -> Result<(), Error> {
        let mut num_bytes_read = 0;

        let mut buf = self.fill_buf().map_err(|_| {
            Error::StaticError("The underlying reader was read, but returned an error.")
        })?;
        let mut size_buf = buf.len() as u64;
        let mut is_eof = buf.is_empty();

        while !is_eof && size_buf < (num_bytes_to_read - num_bytes_read) {
            self.consume(size_buf as usize);
            num_bytes_read += size_buf;

            buf = self.fill_buf().map_err(|_| {
                Error::StaticError("The underlying reader was read, but returned an error.")
            })?;
            size_buf = buf.len() as u64;

            is_eof = buf.is_empty();
        }

        if is_eof {
            if num_bytes_to_read != num_bytes_read {
                // EOF and the num of bytes to read has not been reached
                Err(Error::StaticError("The file has reached EOF"))
            } else {
                // EOF buf all the bytes do have been read
                Ok(())
            }
        } else {
            // Not EOF, we consume the remainig bytes
            let amt = (num_bytes_to_read - num_bytes_read) as usize;
            self.consume(amt);

            Ok(())
        }
    }
}

use async_trait::async_trait;
use futures::io::AsyncBufRead;
use futures::AsyncBufReadExt;

use super::header::extension::asciitable::AsciiTable;
use super::header::extension::bintable::BinTable;
use super::header::extension::image::Image;

/*
#[async_trait(?Send)]
pub trait DataAsyncBufRead<X>: AsyncBufRead + Unpin
where
    X: Xtension,
{
    type Data: Debug;

    fn new_data_block(&mut self, ctx: &X) -> Self::Data
    where
        Self: Sized;

    /// Consume the data to return back the reader at the position
    /// of the end of the data block
    ///
    /// If the data has not been fully read, we skip the remaining data
    /// bytes to go to the end of the data block
    ///
    /// # Params
    /// * `data` - a reader created i.e. from the opening of a file
    async fn consume_data_block(
        &mut self,
        data: Self::Data,
        num_bytes_read: &mut u64,
    ) -> Result<&mut Self, Error>
    where
        Self: Sized;

    async fn read_n_bytes_exact(&mut self, num_bytes_to_read: u64) -> Result<(), Error> {
        let mut num_bytes_read = 0;

        let mut buf = self.fill_buf().await.map_err(|_| {
            Error::StaticError("The underlying reader was read, but returned an error.")
        })?;
        let mut size_buf = buf.len() as u64;
        let mut is_eof = buf.is_empty();

        while !is_eof && size_buf < (num_bytes_to_read - num_bytes_read) {
            self.consume_unpin(size_buf as usize);
            num_bytes_read += size_buf;

            buf = self.fill_buf().await.map_err(|_| {
                Error::StaticError("The underlying reader was read, but returned an error.")
            })?;
            size_buf = buf.len() as u64;

            is_eof = buf.is_empty();
        }

        if is_eof {
            if num_bytes_to_read != num_bytes_read {
                // EOF and the num of bytes to read has not been reached
                Err(Error::StaticError("The file has reached EOF"))
            } else {
                // EOF buf all the bytes do have been read
                Ok(())
            }
        } else {
            // Not EOF, we consume the remainig bytes
            let amt = (num_bytes_to_read - num_bytes_read) as usize;
            self.consume_unpin(amt);

            Ok(())
        }
    }
}*/

/// The full slice of data found in-memory
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataBorrowed` structure
/// for in-memory readers (typically for `&[u8]` or a `Cursor<AsRef<[u8]>>`) that ensures
/// all the data fits in memory
///
#[derive(Serialize, Debug)]
pub struct InMemoryData<'a>(Slice<'a>);

#[derive(Serialize, Debug, Clone)]
pub enum Slice<'a> {
    U8(&'a [u8]),
    I16(&'a [i16]),
    I32(&'a [i32]),
    I64(&'a [i64]),
    F32(&'a [f32]),
    F64(&'a [f64]),
}
