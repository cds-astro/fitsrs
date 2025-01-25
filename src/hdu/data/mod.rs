pub mod asciitable;
pub mod bintable;
pub mod image;
pub mod iter;
pub mod stream;

pub use image::ImageData;
pub use bintable::TableData;

use std::fmt::Debug;

use std::marker::Unpin;

use crate::error::Error;
use crate::hdu::header::Xtension;

use std::io::Read;
pub use stream::DataStream;

/// Special Read trait on top of the std Read trait
/// 
/// This defines methods targeted on reading Fits data units
pub trait FitsRead<'a, X>: Read + Sized
where
    X: Xtension,
{
    /// The type of the returned data.
    /// Usually an iterator over the data
    type Data: Debug + 'a;

    /// Read the data unit providing a special iteratior in function of the extension encountered
    /// 
    /// * Params
    /// 
    /// * ctx - The context of the extension
    /// * start_pos - Information variable telling at which byte position the data starts
    fn read_data_unit(&'a mut self, ctx: &X, start_pos: u64) -> Self::Data
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