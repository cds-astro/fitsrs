pub mod header;

pub mod data;
pub mod extension;
pub mod primary;

use std::convert::TryFrom;

use crate::card::Card;
use crate::card::Value;
use crate::hdu::data::DataBufRead;

//use self::data::DataAsyncBufRead;
use crate::error::Error;

use self::data::AsyncDataBufRead;
use self::header::consume_next_card_async;
use self::header::extension::asciitable::AsciiTable;
use self::header::extension::bintable::BinTable;
use self::header::extension::image::Image;
use self::header::extension::XtensionType;
use self::primary::check_card_keyword;

//use super::data::DataAsyncBufRead;
//use super::AsyncHDU;
use crate::async_fits;
use crate::fits;
use crate::hdu::primary::consume_next_card;

#[derive(Debug)]
pub enum HDU {
    Primary(fits::HDU<Image>),
    XImage(fits::HDU<Image>),
    XBinaryTable(fits::HDU<BinTable>),
    XASCIITable(fits::HDU<AsciiTable>),
}

impl HDU {
    pub(crate) fn new_xtension<'a, R>(reader: &mut R) -> Result<Self, Error>
    where
        R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable> + 'a,
    {
        let mut num_bytes_read = 0;

        let mut card_80_bytes_buf = [0; 80];

        // XTENSION
        consume_next_card(reader, &mut card_80_bytes_buf, &mut num_bytes_read)?;

        let hdu = match Card::try_from(&card_80_bytes_buf)? {
            Card::Extension(XtensionType::Image) => HDU::XImage(fits::HDU::<Image>::new(
                reader,
                &mut num_bytes_read,
                &mut card_80_bytes_buf,
            )?),
            Card::Extension(XtensionType::BinTable) => HDU::XBinaryTable(fits::HDU::<BinTable>::new(
                reader,
                &mut num_bytes_read,
                &mut card_80_bytes_buf,
            )?),
            Card::Extension(XtensionType::AsciiTable) => HDU::XASCIITable(fits::HDU::<AsciiTable>::new(
                reader,
                &mut num_bytes_read,
                &mut card_80_bytes_buf,
            )?),
            _ => {
                let kw = String::from_utf8_lossy(&card_80_bytes_buf[0..8]).into_owned();
                return Err(Error::FailTypeCardParsing(kw, "XTENSION".to_owned()))
            }

        };

        Ok(hdu)
    }

    pub(crate) fn new_primary<'a, R>(reader: &mut R) -> Result<Self, Error>
    where
        //R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable> + 'a,
        R: DataBufRead<'a, Image> + 'a,
    {
        let mut num_bytes_read = 0;
        let mut card_80_bytes_buf = [0; 80];

        // SIMPLE
        consume_next_card(reader, &mut card_80_bytes_buf, &mut num_bytes_read)?;
        if let Value::Logical { value: false, .. } = check_card_keyword(&card_80_bytes_buf, b"SIMPLE  ")? {
            return Err(Error::StaticError("not a FITSv4 file"))
        }

        let hdu = fits::HDU::<Image>::new(reader, &mut num_bytes_read, &mut card_80_bytes_buf)?;

        Ok(HDU::Primary(hdu))
    }
}

#[derive(Debug)]
pub enum AsyncHDU {
    Primary(async_fits::AsyncHDU<Image>),
    XImage(async_fits::AsyncHDU<Image>),
    XBinaryTable(crate::async_fits::AsyncHDU<BinTable>),
    XASCIITable(crate::async_fits::AsyncHDU<AsciiTable>),
}

impl AsyncHDU {
    pub(crate) async fn new_xtension<'a, R>(reader: &mut R) -> Result<Self, Error>
    where
        R: AsyncDataBufRead<'a, Image>
            + AsyncDataBufRead<'a, BinTable>
            + AsyncDataBufRead<'a, AsciiTable>
            + 'a,
    {
        let mut num_bytes_read = 0;

        let mut card_80_bytes_buf = [0; 80];

        // XTENSION
        consume_next_card_async(reader, &mut card_80_bytes_buf, &mut num_bytes_read).await?;

        let hdu = match Card::try_from(&card_80_bytes_buf)? {
            Card::Extension(XtensionType::Image) => AsyncHDU::XImage(
                async_fits::AsyncHDU::<Image>::new(
                    reader,
                    &mut num_bytes_read,
                    &mut card_80_bytes_buf,
                )
                .await?,
            ),
            Card::Extension(XtensionType::BinTable) => AsyncHDU::XBinaryTable(
                async_fits::AsyncHDU::<BinTable>::new(
                    reader,
                    &mut num_bytes_read,
                    &mut card_80_bytes_buf,
                )
                .await?,
            ),
            Card::Extension(XtensionType::AsciiTable) => AsyncHDU::XASCIITable(
                async_fits::AsyncHDU::<AsciiTable>::new(
                    reader,
                    &mut num_bytes_read,
                    &mut card_80_bytes_buf,
                )
                .await?,
            ),
            _ => {
                let kw = String::from_utf8_lossy(&card_80_bytes_buf[0..8]).into_owned();
                return Err(Error::FailTypeCardParsing(kw, "XTENSION".to_owned()))
            }
        };

        Ok(hdu)
    }

    pub(crate) async fn new_primary<'a, R>(reader: &mut R) -> Result<Self, Error>
    where
        //R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable> + 'a,
        R: AsyncDataBufRead<'a, Image> + 'a,
    {
        let mut num_bytes_read = 0;
        let mut card_80_bytes_buf = [0; 80];

        // SIMPLE
        consume_next_card_async(reader, &mut card_80_bytes_buf, &mut num_bytes_read).await?;
        if let Value::Logical { value: false, .. } = check_card_keyword(&card_80_bytes_buf, b"SIMPLE  ")? {
            return Err(Error::StaticError("not a FITSv4 file"))
        }

        let hdu =
            async_fits::AsyncHDU::<Image>::new(reader, &mut num_bytes_read, &mut card_80_bytes_buf)
                .await?;

        Ok(AsyncHDU::Primary(hdu))
    }
}
