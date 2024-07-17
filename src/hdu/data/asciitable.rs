use async_trait::async_trait;
use futures::AsyncRead;
use std::cell::UnsafeCell;
use std::fmt::Debug;
use std::io::{BufReader, Cursor, Read};

use crate::error::Error;

use super::iter;
use super::stream;
use super::{Data, InMemData};

use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::DataBufRead;

use crate::hdu::header::extension::Xtension;

use super::DataAsyncBufRead;

impl<'a, R> DataBufRead<'a, AsciiTable> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + Read + 'a,
{
    type Data = Data<'a, Self>;

    fn new_data_block(&'a mut self, ctx: &AsciiTable) -> Self::Data
    where
        Self: Sized,
    {
        let num_bytes_read = ctx.get_num_bytes_data_block() as usize;

        let bytes = self.get_ref();
        let bytes = bytes.as_ref();

        let pos = self.position() as usize;
        let start_byte_pos = pos;
        let end_byte_pos = pos + num_bytes_read;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        let w = bytes as *const [u8] as *mut UnsafeCell<[u8]>;
        unsafe {
            let word: &UnsafeCell<[u8]> = &*w;
            let x_mut_ref = &mut *word.get();

            let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
            let data = &data[..num_bytes_read];

            Data {
                data: InMemData::U8(data),
                reader: self,
                num_bytes_read,
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

impl<'a, R> DataBufRead<'a, AsciiTable> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = iter::Iter<'a, Self, u8>;

    fn new_data_block(&'a mut self, ctx: &AsciiTable) -> Self::Data {
        let num_bytes_to_read = ctx.get_num_bytes_data_block();
        iter::Iter::new(self, num_bytes_to_read)
    }

    fn consume_data_block(
        data: Self::Data,
        num_bytes_read: &mut u64,
    ) -> Result<&'a mut Self, Error> {
        let iter::Iter {
            reader,
            num_bytes_read: num_bytes_already_read,
            num_bytes_to_read,
            ..
        } = data;

        let remaining_bytes_to_read = num_bytes_to_read - num_bytes_already_read;
        <Self as DataBufRead<'_, AsciiTable>>::read_n_bytes_exact(reader, remaining_bytes_to_read)?;

        // All the data block have been read
        *num_bytes_read = num_bytes_to_read;

        Ok(reader)
    }
}

#[async_trait(?Send)]
impl<'a, R> DataAsyncBufRead<'a, AsciiTable> for futures::io::BufReader<R>
where
    R: AsyncRead + Debug + 'a + std::marker::Unpin,
{
    type Data = stream::St<'a, Self, u8>;

    fn new_data_block(&'a mut self, ctx: &AsciiTable) -> Self::Data {
        let num_bytes_to_read = ctx.get_num_bytes_data_block();
        stream::St::new(self, num_bytes_to_read)
    }

    async fn consume_data_block(
        data: Self::Data,
        num_bytes_read: &mut u64,
    ) -> Result<&'a mut Self, Error>
    where
        'a: 'async_trait,
    {
        let stream::St {
            reader,
            num_bytes_to_read,
            num_bytes_read: num_bytes_already_read,
            ..
        } = data;

        let remaining_bytes_to_read = num_bytes_to_read - num_bytes_already_read;
        <Self as DataAsyncBufRead<'_, AsciiTable>>::read_n_bytes_exact(
            reader,
            remaining_bytes_to_read,
        )
        .await?;

        // All the data block have been read
        *num_bytes_read = num_bytes_to_read;

        Ok(reader)
    }
}
