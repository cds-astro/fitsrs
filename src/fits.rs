pub use crate::hdu::HDU;
use crate::hdu::{Header, DataRead};
use serde::Serialize;
#[derive(Debug)]
pub struct Fits<'a, R>
where
    R: DataRead<'a>
{
    pub hdu: HDU<'a, R>,
}

use crate::error::Error;
impl<'a, R> Fits<'a, R>
where
    R: DataRead<'a> + 'a
{
    /// Parse a FITS file correctly aligned in memory
    ///
    /// # Arguments
    ///
    /// * `buf` - a slice located at a aligned address location with respect to the type T.
    ///   If T is f32, buf ptr must be divisible by 4
    pub fn from_byte_slice(reader: R) -> Result<Self, Error> {
        let hdu = HDU::new(reader)?;

        Ok(Self { hdu })
    }

    pub fn get_header(&self) -> &Header {
        &self.hdu.header
    }

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
        let fits = Fits::from_byte_slice(&mut reader).unwrap();
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
        let fits = Fits::from_byte_slice(&mut reader).unwrap();
        let header = fits.get_header();
        match fits.get_data() {
            DataBorrowed::I16(data) => {
                assert!(data.len() == header.get_axis_size(1).unwrap() * header.get_axis_size(2).unwrap())
            },
            _ => unreachable!(),
        }
    }
}
