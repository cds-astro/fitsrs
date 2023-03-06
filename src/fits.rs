use crate::hdu::extension::XtensionHDU;
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

    pub fn get_primary_hdu(self) -> PrimaryHDU<'a, R> {
        self.hdu
    }

    pub fn get_xtension_hdu(self, mut idx: usize) -> Result<XtensionHDU<'a, R>, Error> {
        let mut hdu_ext = self.hdu.next()?;
        if idx == 0 {
            if let Some(hdu) = hdu_ext {
                Ok(hdu)
            } else {
                Err(Error::StaticError("No more ext HDU found"))
            }
        } else {
            while let Some(hdu) = hdu_ext {
                hdu_ext = hdu.next()?;
                idx -= 1;

                if idx == 0 {
                    break;
                }
            }

            if let Some(hdu) = hdu_ext {
                Ok(hdu)
            } else {
                Err(Error::StaticError("No more ext HDU found"))
            }
        }
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