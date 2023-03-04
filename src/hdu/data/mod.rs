pub mod asciitable;
pub mod bintable;
pub mod image;

use std::fmt::Debug;
use std::io::BufRead;
use crate::hdu::Xtension;
use crate::error::Error;

pub trait DataBufRead<'a, X>: BufRead
where
    X: Xtension
{
    type Data: Debug;

    fn new_data_block(&'a mut self, ctx: &X) -> Self::Data where Self: Sized;

    /// Consume the data to return back the reader at the position
    /// of the end of the data block
    /// 
    /// If the data has not been fully read, we skip the remaining data
    /// bytes to go to the end of the data block
    /// 
    /// # Params
    /// * `data` - a reader created i.e. from the opening of a file
    fn consume_data_block(data: Self::Data, num_bytes_read: &mut usize) -> Result<&'a mut Self, Error>;

    fn read_n_bytes_exact(&mut self, num_bytes_to_read: usize) -> Result<(), Error> {
        let mut num_bytes_read = 0;

        let mut buf = self.fill_buf()
            .map_err(|_| Error::StaticError("The underlying reader was read, but returned an error."))?;
        let mut size_buf = buf.len();
        let mut is_eof = buf.is_empty();

        while !is_eof && size_buf < (num_bytes_to_read - num_bytes_read) {
            self.consume(size_buf);
            num_bytes_read += size_buf;

            buf = self.fill_buf()
                .map_err(|_| Error::StaticError("The underlying reader was read, but returned an error."))?;
            size_buf = buf.len();

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
            self.consume(num_bytes_to_read - num_bytes_read);

            Ok(())
        }
    }
}