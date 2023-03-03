pub mod image;
pub mod bintable;
pub mod asciitable;

use std::io::Read;

use crate::error::Error;
use crate::hdu::primary::check_card_keyword;

pub enum XtensionType {
    Image,
    BinTable,
    AsciiTable,
}

pub fn parse_xtension_card(card: &[u8; 80]) -> Result<XtensionType, Error> {
    let xtension = check_card_keyword(card, b"XTENSION")?.check_for_string()?;
    match xtension.as_str() {
        "IMAGE   " | "IUEIMAGE" => Ok(XtensionType::Image),
        "TABLE   " => Ok(XtensionType::AsciiTable),
        "BINTABLE" => Ok(XtensionType::BinTable),
        _ => Err(Error::NotSupportedXtensionType(xtension))
    }
}

pub trait Xtension {
    fn get_num_bytes_data_block(&self) -> usize;

    // Parse the Xtension keywords
    // During the parsing, some checks will be made
    fn parse<R: Read>(reader: &mut R, num_bytes_read: &mut usize, card_80_bytes_buf: &mut [u8; 80]) -> Result<Self, Error>
        where Self: Sized;
}