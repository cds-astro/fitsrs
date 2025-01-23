pub mod asciitable;
pub mod bintable;
pub mod image;

use std::io::Read;

use async_trait::async_trait;
use futures::AsyncRead;
use serde::Serialize;

use crate::card::Value;
use crate::error::Error;

use std::collections::HashMap;

use super::Card;

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize)]
pub enum XtensionType {
    Image,
    BinTable,
    AsciiTable,
}

impl From<XtensionType> for String {
    fn from(val: XtensionType) -> Self {
        match val {
            XtensionType::Image => "IMAGE".to_owned(),
            XtensionType::BinTable => "BINTABLE".to_owned(),
            XtensionType::AsciiTable => "TABLE".to_owned(),
        }
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
