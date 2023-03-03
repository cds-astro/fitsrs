#[derive(Debug, PartialEq)]
pub enum Error {
    CardSizeNotRespected(usize),
    Not80BytesMultipleFile,
    SimpleKeywordBadValue,
    NomError,
    BitpixBadValue,
    NaxisBadValue,
    NaxisSizeBadValue,
    NaxisSizeNotFound,
    FailReadingNextBytes,
    FailFindingKeyword,
    NegativeOrNullNaxis,
    ValueBadParsing,
    NotSupportedXtensionType(String),
    NegativeOrNullNaxisSize(usize),
    Utf8Error(std::str::Utf8Error),
    StaticError(&'static str),
}

use std::fmt;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::CardSizeNotRespected(_) => write!(f, "card size not repected"),
            Error::Utf8Error(e) => write!(f, "{}", e),
            // TODO
            _ => write!(f, "")
        }
    }
}

impl std::error::Error for Error {}

impl<'a> From<nom::Err<nom::error::Error<&'a [u8]>>> for Error {
    fn from(_: nom::Err<nom::error::Error<&'a [u8]>>) -> Self {
        Error::NomError
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(err: std::str::Utf8Error) -> Self {
        Error::Utf8Error(err)
    }
}

impl From<&'static str> for Error {
    fn from(err: &'static str) -> Self {
        Error::StaticError(err)
    }
}
