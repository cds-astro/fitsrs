use std::borrow::Cow;
#[derive(Debug, PartialEq)]
pub enum Error<'a> {
    CardSizeNotRespected(usize),
    MandatoryKeywordMissing(Cow<'a, str>),
    MustBe8BytesLong(&'a [u8]),
    NomError(nom::Err<nom::error::Error<&'a [u8]>>),
    SimpleKeywordBadValue,
    BitpixBadValue,
    NaxisBadValue,
    NaxisSizeBadValue,
    NaxisSizeNotFound,
    MandatoryValueError(&'a str),
    NegativeOrNullNaxis,
    NegativeOrNullNaxisSize(usize),
}

impl<'a> From<nom::Err<nom::error::Error<&'a [u8]>>> for Error<'a> {
    fn from(nom_err: nom::Err<nom::error::Error<&'a [u8]>>) -> Self {
        Error::NomError(nom_err)
    }
}
