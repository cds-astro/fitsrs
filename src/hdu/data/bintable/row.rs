use crate::hdu::header::extension::bintable::BinTable;
use serde::de::DeserializeOwned;

use std::fmt::Debug;
use std::io::{Read, Seek};

use super::data::TableData;
use super::deser::RowDeserializeIter;
use super::DataValue;
use std::cell::OnceCell;

#[derive(Debug)]
pub struct TableRowData<R> {
    data: TableData<R>,
    idx_row: usize,
}

impl<R> TableRowData<R> {
    pub fn new(data: TableData<R>) -> Self {
        Self { data, idx_row: 0 }
    }

    pub fn get_ctx(&self) -> &BinTable {
        self.data.get_ctx()
    }

    pub fn get_row_idx(&self) -> usize {
        self.idx_row
    }

    pub(crate) fn get_reader(&mut self) -> &mut R {
        self.data.get_reader()
    }

    /// Get an iterator over the binary table without interpreting its content as
    /// a compressed tile.
    ///
    /// This can be useful if you want to have access to the raw data because [TableData] has a method
    /// to get its raw_bytes
    pub fn table_data(self) -> TableData<R> {
        self.data
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
    R: Read + Seek + Debug,
{
    pub fn deserialize<T: DeserializeOwned>(self) -> RowDeserializeIter<R, T> {
        let ttypes = self.get_ctx().ttypes.clone();
        let num_rows = self.get_ctx().naxis2 as usize;

        RowDeserializeIter {
            data: self.data, // or self.table_data() if that's the accessor for the inner TableData<R>
            ttypes,
            row_idx: 0,
            num_rows,
            sorted_fields: OnceCell::new(),
            _marker: std::marker::PhantomData,
        }
    }
}
