use async_trait::async_trait;
use futures::AsyncReadExt;
use std::fmt::Debug;
use std::io::Read;

use super::iter::BigEndianIt;
use super::{stream::St, AsyncDataBufRead};

use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::Xtension;
use crate::hdu::data::FitsRead;

impl<'a, R> FitsRead<'a, AsciiTable> for R
where
    R: Read + Debug + 'a,
{
    type Data = BigEndianIt<&'a mut Self, u8>;

    fn read_data_unit(&'a mut self,
        ctx: &AsciiTable,
        _start_pos: u64
    ) -> Self::Data {
        let limit = ctx.get_num_bytes_data_block() as u64;
        
        BigEndianIt::new(self, limit)
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
