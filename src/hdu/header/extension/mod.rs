pub mod asciitable;
pub mod bintable;
pub mod image;

use std::str::FromStr;

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

impl XtensionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            XtensionType::Image => "IMAGE",
            XtensionType::BinTable => "BINTABLE",
            XtensionType::AsciiTable => "TABLE",
        }
    }
}

impl std::fmt::Display for XtensionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for XtensionType {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
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
