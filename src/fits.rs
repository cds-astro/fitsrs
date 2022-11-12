pub use crate::hdu::HDU;
use crate::hdu::{ Header, Data, DataRead};
use serde::Serialize;
#[derive(Serialize)]
#[derive(Debug)]
pub struct Fits<'a, R>
where
    R: DataRead<'a>
{
    hdu: HDU<'a, R>,
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
    pub unsafe fn from_byte_slice(reader: R) -> Result<Self, Error> {
        let hdu = HDU::new(reader)?;

        Ok(Self { hdu })
    }

    pub fn get_header(&self) -> &Header {
        &self.hdu.header
    }

    pub fn get_data(&self) -> &Data<'_, R> {
        &self.hdu.data
    }
}

#[cfg(test)]
mod tests {
    use super::Fits;
    use std::io::Read;
    use std::io::BufReader;
    use std::io::Cursor;
    use std::fs::File;

    #[test]
    fn test_fits_f32() {
        let mut f = File::open("misc/Npix208.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let mut reader = Cursor::new(&raw_bytes[..]);
        unsafe {
            let fits = Fits::from_byte_slice(reader).unwrap();
            match fits.get_data() {
                DataBorrowed::F32(data) => {
                    assert!(data.len() == hdu.get_axis_size(0).unwrap() * hdu.get_axis_size(1).unwrap())
                },
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_fits_i16() {
        let mut f = File::open("misc/Npix4906.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let mut reader = Cursor::new(&raw_bytes[..]);
        unsafe {
            let Fits { data, hdu } = Fits::from_byte_slice(reader).unwrap();
            match data {
                DataTypeBorrowed::I16(data) => {
                    assert!(data.len() == hdu.get_axis_size(0).unwrap() * hdu.get_axis_size(1).unwrap())
                },
                _ => unreachable!(),
            }
        }
    }
}
