use async_trait::async_trait;
use futures::AsyncReadExt;
use std::io::Read;
use std::{fmt::Debug, io::BufReader};

use super::{stream::St, AsyncDataBufRead};

use crate::hdu::data::FitsRead;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::Xtension;
use crate::hdu::header::Header;

use std::io::Take;
impl<'a, R> FitsRead<'a, AsciiTable> for R
where
    R: Read + Debug + 'a,
{
    type Data = BufReader<Take<&'a mut R>>;

    fn read_data_unit(&'a mut self, header: &Header<AsciiTable>, _start_pos: u64) -> Self::Data {
        let limit = header.get_xtension().get_num_bytes_data_block();

        BufReader::new(self.take(limit))
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
