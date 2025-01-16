use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{BufReader, Seek};
use std::path::Path;
use crate::hdu::data::bintable::RowIt;
use crate::hdu::data::iter::It;
use crate::hdu::data::{DataIter, DataRead};
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;
use crate::Fits;

#[derive(Debug)]
pub enum FITSFile {
    GzFile(GzDecoder<BufReader<File>>),
    File(BufReader<File>),
}

impl Read for FITSFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            FITSFile::GzFile(f) => f.read(buf),
            FITSFile::File(f) => f.read(buf)
        }
    }
}

use std::fmt::Debug;
use std::io::Read;

impl<'a> DataRead<'a, Image> for FITSFile {
    type Data = DataIter<'a, Self>;

    fn init_data_reading_process(
        ctx: &Image,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        DataIter::new(ctx, num_remaining_bytes_in_cur_hdu, reader)
    }
}
impl<'a> DataRead<'a, BinTable> for FITSFile {
    type Data = RowIt<'a, Self>;

    fn init_data_reading_process(
        ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        RowIt::new(reader, ctx, num_remaining_bytes_in_cur_hdu)
    }
}
impl<'a> DataRead<'a, AsciiTable> for FITSFile {
    type Data = It<'a, Self, u8>;

    fn init_data_reading_process(
        _ctx: &AsciiTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        It::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}

use crate::error::Error;
impl FITSFile {
    /// Open a fits file from a path. Can be gzip-compressed
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Fits<Self>, Error> {
        let f = File::open(path)?;

        let bufreader = BufReader::new(f);
        let gz = GzDecoder::new(bufreader);

        match gz.header() {
            // `path` points to a file that is gzip-compressed.
            Some(_) => Ok(Fits::from_reader(FITSFile::GzFile(gz))),
            // `path` points to a plain text file.
            None => {
                let mut f = gz.into_inner();
                // Since the `GzDecoder` already moved some bytes out of f
                // by trying to decompress it, the file must be rewinded
                // TODO There may be a better way instead of reading the same
                // file twice.
                let _ = f.rewind()?;

                Ok(Fits::from_reader(FITSFile::File(f)))
            }
        }
    }
}
