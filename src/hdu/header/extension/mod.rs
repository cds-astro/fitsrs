pub mod asciitable;
pub mod bintable;
pub mod image;

use std::convert::TryFrom;

use async_trait::async_trait;
use serde::Serialize;

use super::ValueMap;
use crate::error::Error;

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

impl TryFrom<&str> for XtensionType {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "IMAGE" | "IUEIMAGE" => Ok(XtensionType::Image),
            "TABLE" => Ok(XtensionType::AsciiTable),
            "BINTABLE" => Ok(XtensionType::BinTable),
            _ => Err(Error::NotSupportedXtensionType(value.to_owned())),
        }
    }
}

#[async_trait(?Send)]
pub trait Xtension {
    /// Return the total size in bytes of the data area
    fn get_num_bytes_data_block(&self) -> u64;

    // Parse the Xtension keywords
    // During the parsing, some checks will be made
    fn parse(values: &ValueMap) -> Result<Self, Error>
    where
        Self: Sized;
}
