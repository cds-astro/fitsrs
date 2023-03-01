use crate::hdu::HDU;
use crate::hdu::{Header, DataRead};
#[derive(Debug)]
pub struct Fits<'a, R>
where
    R: DataRead<'a>
{
    pub hdu: Vec<HDU<'a, R>>,
}

use crate::error::Error;
impl<'a, R> Fits<'a, R>
where
    R: DataRead<'a> + 'a
{
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a reader created i.e. from the opening of a file
    pub fn from_reader(reader: &'a mut R) -> Result<Self, Error> {
        let hdu = HDU::new(reader)?;

        Ok(Self { hdu: vec![hdu] })
    }

    /// Returns the header of the first HDU
    pub fn get_header(&self) -> &Header {
        &self.hdu[0].header
    }

    /// Returns the data of the first HDU
    pub fn get_data(&self) -> &R::Data {
        &self.hdu[0].data
    }
}

use crate::hdu::{AsyncDataRead, AsyncHDU};
#[derive(Debug)]
pub struct AsyncFits<R>
where
    R: AsyncDataRead
{
    pub hdu: AsyncHDU<R>,
}

impl<R> AsyncFits<R>
where
    R: AsyncDataRead + std::marker::Unpin
{
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a async reader created i.e. from the opening of a file
    pub async fn from_reader(reader: R) -> Result<Self, Error> {
        let hdu = AsyncHDU::new(reader).await?;

        Ok(Self { hdu })
    }

    /// Returns the header of the first HDU
    pub fn get_header(&self) -> &Header {
        &self.hdu.header
    }

    /// Returns the data of the first HDU
    pub fn get_data(&self) -> &R::Data {
        &self.hdu.data
    }
}

#[cfg(test)]
mod tests {
    use super::Fits;
    use std::io::Read;
    use crate::hdu::data::DataBorrowed;
    use std::io::Cursor;
    use std::fs::File;

    #[test]
    fn test_fits_f32() {
        let mut f = File::open("misc/Npix208.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let mut reader = Cursor::new(&raw_bytes[..]);
        let fits = Fits::from_reader(&mut reader).unwrap();
        let header = fits.get_header();
        match fits.get_data() {
            DataBorrowed::F32(data) => {
                assert!(data.len() == header.get_axis_size(1).unwrap() * header.get_axis_size(2).unwrap())
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_fits_i16() {
        let mut f = File::open("misc/Npix4906.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let mut reader = Cursor::new(&raw_bytes[..]);
        let fits = Fits::from_reader(&mut reader).unwrap();
        let header = fits.get_header();
        match fits.get_data() {
            DataBorrowed::I16(data) => {
                assert!(data.len() == header.get_axis_size(1).unwrap() * header.get_axis_size(2).unwrap())
            },
            _ => unreachable!(),
        }
    }
}
