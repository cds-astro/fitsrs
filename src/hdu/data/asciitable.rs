use async_trait::async_trait;
use futures::AsyncReadExt;
use std::fmt::Debug;
use std::io::{BufReader, Cursor, Read};

use super::iter::{It, Data};
use super::{stream::St, AsyncDataBufRead};

use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::Xtension;
use crate::hdu::DataRead;

impl<'a, R> DataRead<'a, AsciiTable> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + 'a,
{
    type Data = Data<'a>;

    fn new(
        reader: &'a mut Self,
        ctx: &AsciiTable,
        _num_remaining_bytes_in_cur_hdu: &'a mut usize,
    ) -> Self::Data {
        let num_bytes_of_data = ctx.get_num_bytes_data_block() as usize;

        let start_byte_pos = reader.position() as usize;

        let r = reader.get_ref();
        let bytes = r.as_ref();

        let end_byte_pos = start_byte_pos + num_bytes_of_data;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        let num_pixels = num_bytes_of_data as usize;

        debug_assert!(bytes.len() >= num_pixels);

        Data::U8(bytes)
    }
}

impl<'a, R> DataRead<'a, AsciiTable> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = It<'a, Self, u8>;

    fn new(
        reader: &'a mut Self,
        _ctx: &AsciiTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
    ) -> Self::Data {
        It::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}

#[async_trait(?Send)]
impl<'a, R> AsyncDataBufRead<'a, AsciiTable> for futures::io::BufReader<R>
where
    R: AsyncReadExt + 'a + std::marker::Unpin,
{
    type Data = St<'a, Self, u8>;

    fn prepare_data_reading(
        _ctx: &AsciiTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        St::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}
