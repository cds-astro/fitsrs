pub mod header;
pub mod data;
pub mod data_async;
pub use header::Header;

pub use data::DataRead;
pub use data_async::AsyncDataRead;

use std::fmt::Debug;

/// Structure storing the content of one HDU (i.e. Header Data Unit)
/// of a fits file
#[derive(Debug)]
pub struct HDU<'a, R>
where
    R: DataRead<'a>
{
    /// The header part that stores all the cards
    pub header: Header,
    /// The data part
    pub data: R::Data,
}

use crate::error::Error;
impl<'a, R> HDU<'a, R>
where
    R: DataRead<'a>
{
    pub fn new(reader: &'a mut R) -> Result<Self, Error> {
        let mut bytes_read = 0;
        /* 1. Parse the header first */
        let header = Header::parse(reader, &mut bytes_read)?;
        // At this point the header is valid
        let num_pixels = (0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx + 1).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            });
        let bitpix = header.get_bitpix();

        /* 2. Skip the next bytes to a new 2880 multiple of bytes
        This is where the data block should start */
        let off_data_block = 2880 - bytes_read % 2880;
        reader.consume(off_data_block);

        let data = unsafe { reader.read_data_block(bitpix, num_pixels) };

        Ok(Self {
            header,
            data
        })
    }
}

use std::pin::Pin;
use futures::AsyncBufRead;

/// Structure storing the content of one HDU (i.e. Header Data Unit)
/// of a fits file that is opened in an async way
#[derive(Debug)]
pub struct AsyncHDU<'a, R>
where
    R: AsyncDataRead<'a>
{
    /// The header part that stores all the cards
    pub header: Header,
    /// The data part
    pub data: R::Data,
}
impl<'a, R> AsyncHDU<'a, R>
where
    R: AsyncDataRead<'a> + std::marker::Unpin
{
    pub async fn new(mut reader: &'a mut R) -> Result<AsyncHDU<'a, R>, Error> {
        let mut bytes_read = 0;
        /* 1. Parse the header first */
        let header = Header::parse_async(reader, &mut bytes_read).await?;
        // At this point the header is valid
        let num_pixels = (0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx + 1).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            });
        let bitpix = header.get_bitpix();

        /* 2. Skip the next bytes to a new 2880 multiple of bytes
        This is where the data block should start */
        let off_data_block = 2880 - bytes_read % 2880;
        Pin::new(&mut reader).consume(off_data_block);

        let data = unsafe { reader.read_data_block(bitpix, num_pixels) };

        Ok(Self {
            header,
            data
        })
    }
}


mod tests {
    use super::HDU;
    use super::header::BitpixValue;
    use std::io::{Cursor, Read, BufReader};
    use std::fs::File;

    #[test]
    fn test_cursor_lifetime() {
        let mut f = File::open("misc/Npix208.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();
        // Here all the file content is in memory
        let mut reader = Cursor::new(&raw_bytes[..]);
        let hdu = HDU::new(&mut reader).unwrap();

        assert_eq!(hdu.header.get_bitpix(), BitpixValue::F32);
    }

    #[test]
    fn test_file_lifetime() {
        let f = File::open("misc/Npix208.fits").unwrap();
        let mut reader = BufReader::new(f);

        let hdu = {
            HDU::new(&mut reader).unwrap()
        };

        assert_eq!(hdu.header.get_bitpix(), BitpixValue::F32);
    }
}