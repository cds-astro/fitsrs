use flate2::read::GzDecoder;
use std::io::Seek;
/*
use crate::hdu::data::FitsRead;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;
*/
use std::io::Cursor;
#[derive(Debug)]
pub enum GzReader<R> {
    /// If the the file refers to a gzip file then unzip it entirely and store it in-memory
    /// This will later be read and seekable (thanks to Cursor)
    GzReader(Cursor<Box<[u8]>>),
    Reader(R),
}

impl<R> Read for GzReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            GzReader::GzReader(r) => r.read(buf),
            GzReader::Reader(r) => r.read(buf),
        }
    }
}

/// Hack. I implement Seek so that for non externally gzipped files
/// a seek can be done to get to the next hdu.
///
/// For gzipped file, the data has to be read block by block
impl<R> Seek for GzReader<R>
where
    R: Read + Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            GzReader::GzReader(r) => r.seek(pos),
            GzReader::Reader(r) => r.seek(pos),
        }
    }
}

use std::fmt::Debug;
use std::io::Read;

// We only impl DataRead on gzreaders that wraps a bufreader because in-memory cursors
// do not "read" the data block. Instead the bytes are directly retrieved which prevent the GzDecoder to operate...
/*impl<'a, R> FitsRead<'a, Image> for GzReader<BufReader<R>>
where
    R: Read + Debug + 'a,
{
    type Data = DataIter<'a, Self>;

    fn new(reader: &'a mut Self, ctx: &Image) -> Self::Data {
        DataIter::new(ctx, reader)
    }
}


impl<'a, R> FitsRead<'a, BinTable> for GzReader<BufReader<R>>
where
    R: Read + Debug + 'a
{
    type Data = RowIt<'a, Self>;

    fn new(
        reader: &'a mut Self,
        ctx: &BinTable,
    ) -> Self::Data {
        RowIt::new(reader, ctx)
    }
}
impl<'a, R> FitsRead<'a, AsciiTable> for GzReader<BufReader<R>>
where
    R: Read + Debug + 'a
{
    type Data = It<'a, Self, u8>;

    fn read_data_unit(&mut self,
        _ctx: &AsciiTable,
    ) -> Self::Data {
        It::new(self)
    }
}
*/
use crate::error::Error;
use std::io::SeekFrom;
impl<R> GzReader<R>
where
    R: Read + Seek,
{
    /// Open a fits file from a path. Can be gzip-compressed
    pub fn new(reader: R) -> Result<Self, Error> {
        let mut gz = GzDecoder::new(reader);

        match gz.header() {
            // `path` points to a file that is gzip-compressed.
            Some(_) => {
                let mut buf = vec![];
                gz.read_to_end(&mut buf)?;

                Ok(GzReader::GzReader(Cursor::new(buf.into_boxed_slice())))
            }
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
