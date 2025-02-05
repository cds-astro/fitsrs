use std::fmt::Debug;
use std::io::{Read, Seek, SeekFrom};
use crate::error::Error;
use crate::hdu::header::extension::bintable::BinTable;

use super::DataValue;
use super::data::TableData;

pub(crate) struct TableRowData<R> {
    data: TableData<R>,
    idx_row: usize,
}

impl<R> TableRowData<R> {
    pub(crate) fn new(data: TableData<R>) -> Self {
        Self {
            data,
            idx_row: 0,
        }
    }

    pub(crate) fn get_ctx(&self) -> &BinTable {
        self.data.get_ctx()
    }

    pub(crate) fn get_row_idx(&self) -> usize {
        self.idx_row
    }

    pub(crate) fn get_reader(&mut self) -> &mut R {
        self.data.get_reader()
    }
}

impl<R> Iterator for TableRowData<R>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = Box<[DataValue]>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut row_data = vec![];

        while self.data.row_idx == self.idx_row {
            row_data.push(self.data.next()?);
        }

        self.idx_row += 1;

        Some(row_data.into_boxed_slice())
    }
}

impl<R> TableRowData<R>
where
    R: Seek
{
    pub(crate) fn jump_to_location<F>(&mut self, f: F, pos: SeekFrom) -> Result<(), Error>
    where
        F: FnOnce() -> Result<(), Error>
    {
        self.data.jump_to_location(f, pos)
    }
}