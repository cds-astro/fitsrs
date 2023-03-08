use crate::hdu::extension::{AsyncXtensionHDU, XtensionHDU};
use crate::hdu::primary::{AsyncPrimaryHDU, PrimaryHDU};

use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;

use crate::hdu::data::{DataAsyncBufRead, DataBufRead};

#[derive(Debug)]
pub struct Fits<'a, R>
where
    R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable>,
{
    pub hdu: PrimaryHDU<'a, R>,
}

use crate::error::Error;
impl<'a, R> Fits<'a, R>
where
    R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable>,
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

#[derive(Debug)]
pub struct AsyncFits<'a, R>
where
    R: DataAsyncBufRead<'a, Image>
        + DataAsyncBufRead<'a, BinTable>
        + DataAsyncBufRead<'a, AsciiTable>,
{
    pub hdu: AsyncPrimaryHDU<'a, R>,
}

impl<'a, R> AsyncFits<'a, R>
where
    R: DataAsyncBufRead<'a, Image>
        + DataAsyncBufRead<'a, BinTable>
        + DataAsyncBufRead<'a, AsciiTable>
        + std::marker::Send,
{
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a reader created i.e. from the opening of a file
    pub async fn from_reader(reader: &'a mut R) -> Result<AsyncFits<'a, R>, Error> {
        let hdu = AsyncPrimaryHDU::new(reader).await?;

        Ok(Self { hdu })
    }

    pub fn get_primary_hdu(self) -> AsyncPrimaryHDU<'a, R> {
        self.hdu
    }

    pub async fn get_xtension_hdu(self, mut idx: usize) -> Result<AsyncXtensionHDU<'a, R>, Error> {
        let mut hdu_ext = self.hdu.next().await?;
        if idx == 0 {
            if let Some(hdu) = hdu_ext {
                Ok(hdu)
            } else {
                Err(Error::StaticError("No more ext HDU found"))
            }
        } else {
            while let Some(hdu) = hdu_ext {
                hdu_ext = hdu.next().await?;
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
