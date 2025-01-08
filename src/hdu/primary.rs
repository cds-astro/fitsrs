use super::{
    //data::DataAsyncBufRead,
    //extension::{AsyncXtensionHDU, XtensionHDU},
    header::consume_next_card_async,
    //AsyncHDU,
};
use crate::hdu::extension::XtensionHDU;
use std::fmt::Debug;

use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;

use crate::hdu::data::DataBufRead;

use crate::fits::HDU;

use crate::error::Error;
pub use crate::hdu::header::{check_card_keyword, consume_next_card};

use std::ops::Deref;

/// Structure storing the content of one HDU (i.e. Header Data Unit)
/// of a fits file
#[derive(Debug)]
pub struct PrimaryHDU(pub HDU<Image>);

impl PrimaryHDU {
    pub fn new<'a, R>(reader: &mut R, num_bytes_read: &mut usize) -> Result<Self, Error>
    where
        //R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable> + 'a,
        R: DataBufRead<'a, Image> + 'a,
    {
        let mut card_80_bytes_buf = [0; 80];

        // SIMPLE
        consume_next_card(reader, &mut card_80_bytes_buf, num_bytes_read)?;
        let _ = check_card_keyword(&card_80_bytes_buf, b"SIMPLE  ")?;

        let hdu = HDU::<Image>::new(reader, num_bytes_read, &mut card_80_bytes_buf)?;

        Ok(PrimaryHDU(hdu))
    }

    /*fn consume<'a, R>(self, reader: &mut R) -> Result<Option<&mut R>, Error>
    where
        R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable> + 'a,
    {
        self.0.consume(reader)
    }

    pub fn next<'a, R>(self, reader: &'a mut R) -> Result<Option<XtensionHDU>, Error>
    where
        R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable> + 'a,
    {
        if let Some(reader) = self.consume(reader)? {
            let hdu = XtensionHDU::new(reader)?;
            Ok(Some(hdu))
        } else {
            Ok(None)
        }
    }*/
}

impl Deref for PrimaryHDU {
    type Target = HDU<Image>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

use std::ops::DerefMut;
impl DerefMut for PrimaryHDU {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/*
#[derive(Debug)]
pub struct AsyncPrimaryHDU<'a, R>(pub AsyncHDU<'a, R, Image>)
where
    R: DataAsyncBufRead<'a, Image>
        + DataAsyncBufRead<'a, BinTable>
        + DataAsyncBufRead<'a, AsciiTable>
        + 'a;

impl<'a, R> AsyncPrimaryHDU<'a, R>
where
    R: DataAsyncBufRead<'a, Image>
        + DataAsyncBufRead<'a, BinTable>
        + DataAsyncBufRead<'a, AsciiTable>
        + 'a,
{
    pub async fn new(reader: &'a mut R) -> Result<AsyncPrimaryHDU<'a, R>, Error> {
        let mut num_bytes_read = 0;
        let mut card_80_bytes_buf = [0; 80];

        // SIMPLE
        consume_next_card_async(reader, &mut card_80_bytes_buf, &mut num_bytes_read).await?;
        let _ = check_card_keyword(&card_80_bytes_buf, b"SIMPLE  ")?;

        let hdu =
            AsyncHDU::<'a, R, Image>::new(reader, &mut num_bytes_read, &mut card_80_bytes_buf)
                .await?;

        Ok(AsyncPrimaryHDU(hdu))
    }

    async fn consume(self) -> Result<Option<&'a mut R>, Error> {
        self.0.consume().await
    }

    pub async fn next(self) -> Result<Option<AsyncXtensionHDU<'a, R>>, Error> {
        if let Some(reader) = self.consume().await? {
            let hdu = AsyncXtensionHDU::new(reader).await?;
            Ok(Some(hdu))
        } else {
            Ok(None)
        }
    }
}

impl<'a, R> Deref for AsyncPrimaryHDU<'a, R>
where
    R: DataAsyncBufRead<'a, Image>
        + DataAsyncBufRead<'a, BinTable>
        + DataAsyncBufRead<'a, AsciiTable>
        + 'a,
{
    type Target = AsyncHDU<'a, R, Image>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, R> DerefMut for AsyncPrimaryHDU<'a, R>
where
    R: DataAsyncBufRead<'a, Image>
        + DataAsyncBufRead<'a, BinTable>
        + DataAsyncBufRead<'a, AsciiTable>
        + 'a,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}*/

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
