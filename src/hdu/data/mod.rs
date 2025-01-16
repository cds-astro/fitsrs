pub mod asciitable;
pub mod bintable;
pub mod image;
pub mod iter;
pub mod stream;

use serde::Serialize;

use std::fmt::Debug;

use std::marker::Unpin;

use crate::error::Error;
use crate::hdu::header::Xtension;

pub use iter::DataIter;
use std::io::Read;
pub use stream::DataStream;

// reader must impl this
pub trait DataRead<'a, X>: Read + Sized
where
    X: Xtension,
{
    type Data: Debug + 'a;

    fn init_data_reading_process(
        ctx: &X,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data
    where
        Self: Sized;
}

use async_trait::async_trait;
use futures::io::AsyncBufRead;
use futures::AsyncBufReadExt;

#[async_trait(?Send)]
pub trait AsyncDataBufRead<'a, X>: AsyncBufRead + Unpin
where
    X: Xtension,
{
    type Data: 'a;

    fn prepare_data_reading(
        ctx: &X,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data
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
}

/// The full slice of data found in-memory
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataBorrowed` structure
/// for in-memory readers (typically for `&[u8]` or a `Cursor<AsRef<[u8]>>`) that ensures
/// all the data fits in memory
///
use std::borrow::Cow;
#[derive(Serialize, Debug, Clone)]
pub enum Data<'a> {
    U8(Cow<'a, [u8]>),
    I16(Box<[i16]>),
    I32(Box<[i32]>),
    I64(Box<[i64]>),
    F32(Box<[f32]>),
    F64(Box<[f64]>),
}
