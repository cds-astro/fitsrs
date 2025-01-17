use flate2::read::GzDecoder;
//use serde::Serialize;
use std::io::Seek;
use crate::hdu::data::bintable::RowIt;
use crate::hdu::data::iter::It;
use crate::hdu::data::{DataIter, DataRead};
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;

#[derive(Debug)]
pub enum GzReader<R> {
    GzReader(GzDecoder<R>),
    Reader(R),
}

impl<R> Read for GzReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            GzReader::GzReader(r) => r.read(buf),
            GzReader::Reader(r) => r.read(buf)
        }
    }
}

use std::fmt::Debug;
use std::io::Read;

impl<'a, R> DataRead<'a, Image> for GzReader<R>
where
    R: Read + Debug + 'a
{
    type Data = DataIter<'a, Self>;

    fn init_data_reading_process(
        ctx: &Image,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        DataIter::new(ctx, num_remaining_bytes_in_cur_hdu, reader)
    }
}
impl<'a, R> DataRead<'a, BinTable> for GzReader<R>
where
    R: Read + Debug + 'a
{
    type Data = RowIt<'a, Self>;

    fn init_data_reading_process(
        ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        RowIt::new(reader, ctx, num_remaining_bytes_in_cur_hdu)
    }
}
impl<'a, R> DataRead<'a, AsciiTable> for GzReader<R>
where
    R: Read + Debug + 'a
{
    type Data = It<'a, Self, u8>;

    fn init_data_reading_process(
        _ctx: &AsciiTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        It::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}

use std::io::SeekFrom;
use crate::error::Error;
impl<R> GzReader<R>
where
    R: Read + Seek
{
    /// Open a fits file from a path. Can be gzip-compressed
    pub fn new(reader: R) -> Result<Self, Error> {
        let gz = GzDecoder::new(reader);

        match gz.header() {
            // `path` points to a file that is gzip-compressed.
            Some(_) => Ok(GzReader::GzReader(gz)),
            // `path` points to a plain text file.
            None => {
                let mut r = gz.into_inner();
                // Since the `GzDecoder` already moved some bytes out of f
                // by trying to decompress it, the file must be rewinded
                // TODO There may be a better way instead of reading the same
                // file twice.
                r.seek(SeekFrom::Start(0))?;

                Ok(GzReader::Reader(r))
            }
        }
    }
}
