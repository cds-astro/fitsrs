use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{BufReader, Seek};
use std::path::Path;

use crate::fits::HDU;
use crate::hdu::data::bintable::RowIt;
use crate::hdu::data::iter::It;
use crate::hdu::data::{DataIter, DataRead};
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::Xtension;
use crate::Fits;
pub enum FITSFile {
    Gz(Fits<GzDecoder<BufReader<File>>>),
    Plain(Fits<BufReader<File>>),
}

use std::fmt::Debug;
use std::io::Read;

impl<'a, R> DataRead<'a, Image> for GzDecoder<R>
where
    R: Read + Debug + 'a,
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
impl<'a, R> DataRead<'a, BinTable> for GzDecoder<R>
where
    R: Read + Debug + 'a,
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
impl<'a, R> DataRead<'a, AsciiTable> for GzDecoder<R>
where
    R: Read + Debug + 'a,
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

use crate::error::Error;
impl FITSFile {
    /// Open a fits file from a path. Can be gzip-compressed
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let f = File::open(path)?;

        let bufreader = BufReader::new(f);
        let gz = GzDecoder::new(bufreader);

        match gz.header() {
            // `path` points to a file that is gzip-compressed.
            Some(_) => Ok(FITSFile::Gz(Fits::from_reader(gz))),
            // `path` points to a plain text file.
            None => {
                let mut f = gz.into_inner();
                // Since the `GzDecoder` already moved some bytes out of f
                // by trying to decompress it, the file must be rewinded
                // TODO There may be a better way instead of reading the same
                // file twice.
                let _ = f.rewind()?;

                Ok(FITSFile::Plain(Fits::from_reader(f)))
            }
        }
    }

    /*pub fn get_data<'a, R, X: Xtension + Debug>(
        &'a mut self,
        hdu: HDU<X>,
    ) -> <BufReader<File> as DataRead<'a, X>>::Data
    where
        std::io::BufReader<File>: DataRead<'a, X>,
        flate2::read::GzDecoder<std::io::BufReader<File>>: DataRead<'a, X>,
    {
        // Unroll the internal fits parsing parameters to give it to the data reader
        match self {
            FITSFile::Gz(gz) => gz.get_data(hdu),
            FITSFile::Plain(f) => f.get_data(hdu),
        }
    }*/
}

impl Iterator for FITSFile {
    type Item = Result<crate::hdu::HDU, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            FITSFile::Gz(gz) => gz.next(),
            FITSFile::Plain(f) => f.next(),
        }
    }
}
