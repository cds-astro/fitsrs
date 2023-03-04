use super::extension::XtensionHDU;
use std::fmt::Debug;

use crate::hdu::header::extension::image::Image;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;

use crate::hdu::data::DataBufRead;

use crate::hdu::HDU;

/// Structure storing the content of one HDU (i.e. Header Data Unit)
/// of a fits file
#[derive(Debug)]
pub struct PrimaryHDU<'a, R>(pub HDU<'a, R, Image>)
where
    R: DataBufRead<'a, Image> +
       DataBufRead<'a, BinTable> +
       DataBufRead<'a, AsciiTable> +
       'a;

use crate::error::Error;
pub use crate::hdu::{
    header::{
        check_card_keyword,
        consume_next_card,
    }
};

use std::ops::Deref;
impl<'a, R> Deref for PrimaryHDU<'a, R>
where
    R: DataBufRead<'a, Image> +
       DataBufRead<'a, BinTable> +
       DataBufRead<'a, AsciiTable> +
       'a
{
    type Target = HDU<'a, R, Image>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

use std::ops::DerefMut;
impl<'a, R> DerefMut for PrimaryHDU<'a, R>
where
    R: DataBufRead<'a, Image> +
       DataBufRead<'a, BinTable> +
       DataBufRead<'a, AsciiTable> +
       'a
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, R> PrimaryHDU<'a, R>
where
    R: DataBufRead<'a, Image> +
       DataBufRead<'a, BinTable> +
       DataBufRead<'a, AsciiTable> +
       'a
{
    pub fn new(reader: &'a mut R) -> Result<Self, Error> {
        let mut num_bytes_read = 0;
        let mut card_80_bytes_buf = [0; 80];

        // SIMPLE
        consume_next_card(reader, &mut card_80_bytes_buf, &mut num_bytes_read)?;
        let _ = check_card_keyword(&card_80_bytes_buf, b"SIMPLE  ")?;

        let hdu = HDU::<'a, R, Image>::new(reader, &mut num_bytes_read, &mut card_80_bytes_buf)?;

        Ok(PrimaryHDU(hdu))
    }

    fn consume(self) -> Result<Option<&'a mut R>, Error> {
        self.0.consume()
    }

    pub fn next(self) -> Result<Option<XtensionHDU<'a, R>>, Error> {
        if let Some(reader) = self.consume()? { 
            let hdu = XtensionHDU::new(reader)?;
            Ok(Some(hdu))
        } else {
            Ok(None)
        }
    }
}
/*
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
    pub header: PrimaryHeader,
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
        let header = PrimaryHeader::parse_async(reader, &mut bytes_read).await?;
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
*/