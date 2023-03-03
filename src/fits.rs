use crate::hdu::primary::PrimaryHDU;

use crate::hdu::header::extension::image::Image;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;

use crate::hdu::data::DataBufRead;

#[derive(Debug)]
pub struct Fits<'a, R>
where
    R: DataBufRead<'a, Image> +
       DataBufRead<'a, BinTable> +
       DataBufRead<'a, AsciiTable>
{
    pub hdu: PrimaryHDU<'a, R>,
}

use crate::error::Error;
impl<'a, R> Fits<'a, R>
where
    R: DataBufRead<'a, Image> +
       DataBufRead<'a, BinTable> +
       DataBufRead<'a, AsciiTable>
{
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a reader created i.e. from the opening of a file
    pub fn from_reader(reader: &'a mut R) -> Result<Self, Error> {
        let hdu = PrimaryHDU::new(reader)?;

        Ok(Self { hdu })
    }

    /// Returns the header of the first HDU
    pub fn get_first_hdu(&self) -> &PrimaryHDU<'a, R> {
        &self.hdu
    }
}

/* 
use crate::hdu::{AsyncDataRead, AsyncHDU};

#[derive(Debug)]
pub struct AsyncFits<'a, R>
where
    R: AsyncDataRead<'a>
{
    pub hdu: Vec<AsyncHDU<'a, R>>,
}

impl<'a, R> AsyncFits<'a, R>
where
    R: AsyncDataRead<'a> + std::marker::Unpin
{
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a async reader created i.e. from the opening of a file
    pub async fn from_reader(reader: &'a mut R) -> Result<AsyncFits<'a, R>, Error> {
        let hdu = AsyncHDU::new(reader).await?;

        Ok(Self { hdu: vec![hdu] })
    }

    /// Returns the header of the first HDU
    pub fn get_header(&self) -> &PrimaryHeader {
        &self.hdu[0].header
    }

    /// Returns the data of the first HDU
    pub fn get_data(&self) -> &R::Data {
        &self.hdu[0].data
    }
}
*/
#[cfg(test)]
mod tests {
    use super::Fits;
    use std::io::Read;
    use crate::hdu::data::image::DataBorrowed;
    use std::io::Cursor;
    use std::fs::File;
    /*
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
    }*/
}
