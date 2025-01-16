use async_trait::async_trait;
use futures::AsyncReadExt;
use std::fmt::Debug;
use std::io::{BufReader, Cursor, Read};

use super::{iter, Data};
use super::{stream, AsyncDataBufRead};

use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::Xtension;
use crate::hdu::DataRead;
use std::borrow::Cow;

impl<'a, R> DataRead<'a, AsciiTable> for Cursor<R>
where
    R: AsRef<[u8]> + Debug + 'a,
{
    type Data = Data<'a>;

    fn init_data_reading_process(
        ctx: &AsciiTable,
        _num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        let num_bytes_of_data = ctx.get_num_bytes_data_block() as usize;

        let start_byte_pos = reader.position() as usize;

        let r = reader.get_ref();
        let bytes = r.as_ref();

        let end_byte_pos = start_byte_pos + num_bytes_of_data;

        let bytes = &bytes[start_byte_pos..end_byte_pos];

        let num_pixels = num_bytes_of_data as usize;

        debug_assert!(bytes.len() >= num_pixels);

        Data::U8(Cow::Borrowed(bytes))
    }
}

impl<'a, R> DataRead<'a, AsciiTable> for BufReader<R>
where
    R: Read + Debug + 'a,
{
    type Data = iter::It<'a, Self, u8>;

    fn init_data_reading_process(
        _ctx: &AsciiTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        iter::It::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}

#[async_trait(?Send)]
impl<'a, R> AsyncDataBufRead<'a, AsciiTable> for futures::io::BufReader<R>
where
    R: AsyncReadExt + 'a + std::marker::Unpin,
{
    type Data = stream::St<'a, Self, u8>;

    fn prepare_data_reading(
        _ctx: &AsciiTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        stream::St::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}
