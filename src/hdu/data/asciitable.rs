use std::io::{Cursor, BufReader, Read};
use std::fmt::Debug;

use crate::error::Error;

use crate::hdu::DataBufRead;
use crate::hdu::data::image::DataOwnedIt;
use crate::hdu::data::image::InMemData;
use crate::hdu::data::image::DataBorrowed;
use crate::hdu::header::extension::asciitable::AsciiTable;

use crate::hdu::header::extension::Xtension;

impl<'a, R> DataBufRead<'a, AsciiTable> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + Read + 'a
{
    type Data = DataBorrowed<'a, Self>;

    fn new_data_block(&'a mut self, ctx: &AsciiTable) -> Self::Data where Self: Sized {
        let num_bytes_read = ctx.get_num_bytes_data_block();

        let bytes = self.get_ref();
        let bytes = bytes.as_ref();

        let pos = self.position() as usize;
        let start_byte_pos = pos;
        let end_byte_pos = pos + num_bytes_read;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        let x_ptr = bytes as *const [u8] as *mut [u8];
        unsafe {
            let x_mut_ref = &mut *x_ptr;
    
            let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
            let data = &data[..num_bytes_read];

            DataBorrowed {
                data: InMemData::U8(data),
                reader: self,
                num_bytes_read
            }
        }
    }

    fn consume_data_block(data: Self::Data, num_bytes_read: &mut usize) -> Result<&'a mut Self, Error> {
        let DataBorrowed {reader, num_bytes_read: num_bytes, ..} = data;
        *num_bytes_read = num_bytes;

        reader.set_position(reader.position() + num_bytes as u64);

        Ok(reader)
    }
}

impl<'a, R> DataBufRead<'a, AsciiTable> for BufReader<R>
where
    R: Read + Debug + 'a
{
    type Data = DataOwnedIt<'a, Self, u8>;

    fn new_data_block(&'a mut self, ctx: &AsciiTable) -> Self::Data {
        let num_bytes_to_read = ctx.get_num_bytes_data_block();
        DataOwnedIt::new(self, num_bytes_to_read)
    }

    fn consume_data_block(data: Self::Data, num_bytes_read: &mut usize) -> Result<&'a mut Self, Error> {
        let DataOwnedIt { reader, num_bytes_read: num_bytes_already_read, num_bytes_to_read, .. } = data;

        let remaining_bytes_to_read = num_bytes_to_read - num_bytes_already_read;
        <Self as DataBufRead<'_, AsciiTable>>::read_n_bytes_exact(reader, remaining_bytes_to_read)?;

        // All the data block have been read
        *num_bytes_read = num_bytes_to_read;

        Ok(reader)
    }
}