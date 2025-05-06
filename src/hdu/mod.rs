pub mod header;

pub mod data;
pub mod primary;

use std::convert::TryFrom;
use std::io::Seek;

use futures::AsyncRead;

use crate::card::Card;
use crate::card::CardBuf;
use crate::card::Value;

use crate::hdu::data::FitsRead;

//use self::data::DataAsyncBufRead;
use crate::error::Error;

use self::data::AsyncDataBufRead;
use self::header::consume_next_card_async;
use self::header::extension::asciitable::AsciiTable;
use self::header::extension::bintable::BinTable;
use self::header::extension::image::Image;
use self::header::extension::XtensionType;
use crate::hdu::Value::Logical;

//use super::data::DataAsyncBufRead;
//use super::AsyncHDU;
use crate::async_fits;
use crate::fits;
use crate::hdu::primary::consume_next_card;
use log::error;
/// An enumeration of the supported FITS Header Data Unit types.
#[derive(Debug, PartialEq)]
pub enum HDU {
    /// The primary HDU refers to an image
    Primary(fits::HDU<Image>),
    /// HDU image extension
    XImage(fits::HDU<Image>),
    /// HDU binary table extension
    XBinaryTable(fits::HDU<BinTable>),
    /// HDU ASCII table extension
    XASCIITable(fits::HDU<AsciiTable>),
}

use std::io::Read;
fn consume_cards<R>(reader: &mut R, num_bytes_read: &mut usize) -> Result<Vec<Card>, Error>
where
    R: Read,
{
    let mut card_80_bytes_buf: CardBuf = [0; 80];
    let mut cards = Vec::new();

    /* Consume cards until `END` is reached */
    loop {
        consume_next_card(reader, &mut card_80_bytes_buf, num_bytes_read)
            // Precise the error that we did not encounter the END stopping card
            .inspect_err(|_e| {
                error!("Fail reading the header without encountering the END card");
            })?;

        if let Ok(card) = Card::try_from(&card_80_bytes_buf) {
            cards.push(card);
            if Some(&Card::End) == cards.last() {
                break;
            }
        } else {
            // FIXME log warning
            // preserve the unparsable header
            let card = Card::Undefined(String::from_utf8_lossy(&card_80_bytes_buf).into_owned());
            cards.push(card);
        }
    }

    Ok(cards)
}

async fn consume_cards_async<R>(
    reader: &mut R,
    num_bytes_read: &mut usize,
) -> Result<Vec<Card>, Error>
where
    R: AsyncRead + std::marker::Unpin,
{
    let mut card_80_bytes_buf: CardBuf = [0; 80];
    let mut cards = Vec::new();

    /* Consume cards until `END` is reached */
    loop {
        consume_next_card_async(reader, &mut card_80_bytes_buf, num_bytes_read)
            .await
            // Precise the error that we did not encounter the END stopping card
            .map_err(|_| {
                Error::StaticError("Fail reading the header without encountering the END card")
            })?;
        if let Ok(card) = Card::try_from(&card_80_bytes_buf) {
            cards.push(card);
            if Some(&Card::End) == cards.last() {
                break;
            }
        } else {
            // FIXME log warning
            // preserve the unparsable header
            let card = Card::Undefined(String::from_utf8_lossy(&card_80_bytes_buf).into_owned());
            cards.push(card);
        }
    }

    Ok(cards)
}

impl HDU {
    pub(crate) fn new_xtension<'a, R>(
        reader: &mut R,
        num_bytes_read: &mut usize,
    ) -> Result<Self, Error>
    where
        R: FitsRead<'a, Image> + FitsRead<'a, BinTable> + FitsRead<'a, AsciiTable> + Seek + 'a,
    {
        let cards = consume_cards(reader, num_bytes_read)?;
        // Check only the the first card. Even if not FITS valid we could accept
        // it if its xtension card is down in the header.
        match &cards[0] {
            Card::Xtension {
                x: XtensionType::Image,
                ..
            } => Ok(HDU::XImage(fits::HDU::<Image>::new(
                reader,
                num_bytes_read,
                cards,
            )?)),
            Card::Xtension {
                x: XtensionType::BinTable,
                ..
            } => Ok(HDU::XBinaryTable(fits::HDU::<BinTable>::new(
                reader,
                num_bytes_read,
                cards,
            )?)),
            Card::Xtension {
                x: XtensionType::AsciiTable,
                ..
            } => Ok(HDU::XASCIITable(fits::HDU::<AsciiTable>::new(
                reader,
                num_bytes_read,
                cards,
            )?)),
            _ => Err(Error::StaticError(
                "XTENSION card has not been found in the header",
            )),
        }
    }

    pub(crate) fn new_primary<'a, R>(reader: &mut R) -> Result<Self, Error>
    where
        R: FitsRead<'a, Image> + Seek + 'a,
    {
        let mut num_bytes_read = 0;

        let cards = consume_cards(reader, &mut num_bytes_read)?;

        // Check for SIMPLE keyword
        if let Card::Value {
            name,
            value: Logical { value: true, .. },
            ..
        } = &cards[0]
        {
            if name == "SIMPLE" {
                Ok(HDU::Primary(fits::HDU::<Image>::new(
                    reader,
                    &mut num_bytes_read,
                    cards,
                )?))
            } else {
                // TODO log the card to stderr
                Err(Error::DynamicError(format!(
                    "Invalid FITS file: expected `SIMPLE` keyword in first card, found `{name}`"
                )))
            }
        } else {
            // TODO log the card to stderr
            Err(Error::StaticError("not a FITSv4 file"))
        }
    }

    pub fn get_data_unit_byte_offset(&self) -> u64 {
        match self {
            HDU::Primary(hdu) | HDU::XImage(hdu) => hdu.get_data_unit_byte_offset(),
            HDU::XBinaryTable(hdu) => hdu.get_data_unit_byte_offset(),
            HDU::XASCIITable(hdu) => hdu.get_data_unit_byte_offset(),
        }
    }
    pub fn get_data_unit_byte_size(&self) -> u64 {
        match self {
            HDU::Primary(hdu) | HDU::XImage(hdu) => hdu.get_data_unit_byte_size(),
            HDU::XBinaryTable(hdu) => hdu.get_data_unit_byte_size(),
            HDU::XASCIITable(hdu) => hdu.get_data_unit_byte_size(),
        }
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

        let cards = consume_cards_async(reader, &mut num_bytes_read).await?;
        // Check only the if the first card. Even if not FITS valid we could accept
        // it if its xtension card is down in the header.
        let hdu = match &cards[0] {
            Card::Xtension {
                x: XtensionType::Image,
                ..
            } => AsyncHDU::XImage(
                async_fits::AsyncHDU::<Image>::new(reader, &mut num_bytes_read, cards).await?,
            ),
            Card::Xtension {
                x: XtensionType::BinTable,
                ..
            } => AsyncHDU::XBinaryTable(
                async_fits::AsyncHDU::<BinTable>::new(reader, &mut num_bytes_read, cards).await?,
            ),
            Card::Xtension {
                x: XtensionType::AsciiTable,
                ..
            } => AsyncHDU::XASCIITable(
                async_fits::AsyncHDU::<AsciiTable>::new(reader, &mut num_bytes_read, cards).await?,
            ),
            _ => {
                return Err(Error::StaticError(
                    "XTENSION card has not been found in the header",
                ));
            }
        };

        Ok(hdu)
    }

    pub(crate) async fn new_primary<'a, R>(reader: &mut R) -> Result<Self, Error>
    where
        R: AsyncDataBufRead<'a, Image> + 'a,
    {
        let mut num_bytes_read = 0;

        let cards = consume_cards_async(reader, &mut num_bytes_read).await?;

        // Check for SIMPLE keyword
        let _name: String = "SIMPLE".to_owned();
        if let Card::Value {
            name: _name,
            value: Logical { value: true, .. },
            ..
        } = &cards[0]
        {
            Ok(AsyncHDU::Primary(
                async_fits::AsyncHDU::<Image>::new(reader, &mut num_bytes_read, cards).await?,
            ))
        } else {
            Err(Error::StaticError("not a FITSv4 file"))
        }
    }
}
