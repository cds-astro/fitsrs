pub mod asciitable;
pub mod bintable;
pub mod image;

use std::convert::TryFrom;
use std::io::Read;

use async_trait::async_trait;
use futures::AsyncRead;

use crate::card::Value;
use crate::error::Error;

use std::collections::HashMap;

use super::{Card, CardBuf};

#[derive(Debug)]
pub enum XtensionType {
    Image,
    BinTable,
    AsciiTable,
}

pub fn parse_xtension_card(card: &CardBuf) -> Result<XtensionType, Error> {
    let xtension = if let Card::Extension(x) = Card::try_from(card)? {
        x
    } else {
        let kw = String::from_utf8_lossy(&card[..8]).into_owned();
        return Err(Error::FailTypeCardParsing(kw, "XTENSION".to_owned()))
    };

    match &xtension {
        b"IMAGE   " | b"IUEIMAGE" => Ok(XtensionType::Image),
        b"TABLE   " => Ok(XtensionType::AsciiTable),
        b"BINTABLE" => Ok(XtensionType::BinTable),
        _ => Err(Error::NotSupportedXtensionType(String::from_utf8_lossy(&xtension).into_owned())),
    }
}

#[async_trait(?Send)]
pub trait Xtension {
    fn get_num_bytes_data_block(&self) -> u64;

    fn update_with_parsed_header(&mut self, cards: &HashMap<String, Value>) -> Result<(), Error>;

    // Parse the Xtension keywords
    // During the parsing, some checks will be made
    fn parse<R: Read>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
        cards: &mut Vec<Card>,
    ) -> Result<Self, Error>
    where
        Self: Sized;

    // Async equivalent method
    async fn parse_async<R>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
        cards: &mut Vec<Card>,
    ) -> Result<Self, Error>
    where
        Self: Sized,
        R: AsyncRead + std::marker::Unpin;
}
