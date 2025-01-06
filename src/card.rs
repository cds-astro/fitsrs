use std::string::FromUtf8Error;

use nom::{
    character::complete::{i64, space0},
    combinator::map,
    sequence::preceded,
    IResult,
};

use crate::error::Error;
pub type Keyword = [u8; 8];

use serde::Serialize;
#[derive(Debug, PartialEq, Serialize)]
pub struct Card {
    pub kw: Keyword,
    pub v: Value,
}

impl Card {
    pub fn new(kw: Keyword, v: Value) -> Self {
        Self { kw, v }
    }
    /// Return the ASCII trimmed keyword as an owned String.
    pub fn keyword(&self) -> Result<String, FromUtf8Error> {
        let kw = self.kw.trim_ascii().to_vec();
        String::from_utf8(kw)
    }
}

pub trait CardValue {
    fn parse(value: Value) -> Result<Self, Error>
    where
        Self: Sized;
}

impl CardValue for f64 {
    fn parse(value: Value) -> Result<Self, Error> {
        Value::check_for_float(value)
    }
}
impl CardValue for i64 {
    fn parse(value: Value) -> Result<Self, Error> {
        Value::check_for_integer(value)
    }
}
impl CardValue for String {
    fn parse(value: Value) -> Result<Self, Error> {
        Value::check_for_string(value)
    }
}
impl CardValue for bool {
    fn parse(value: Value) -> Result<Self, Error> {
        Value::check_for_boolean(value)
    }
}

/// Enum structure corresponding to all the possible type
/// a card value can have that are supported by fitsrs
#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum Value {
    Integer(i64),
    Logical(bool),
    String(String),
    Float(f64),
    Undefined,
}

impl Value {
    pub fn check_for_integer(self) -> Result<i64, Error> {
        match self {
            Value::Integer(num) => Ok(num),
            _ => Err(Error::ValueBadParsing),
        }
    }
    pub fn check_for_boolean(self) -> Result<bool, Error> {
        match self {
            Value::Logical(logical) => Ok(logical),
            _ => Err(Error::ValueBadParsing),
        }
    }
    pub fn check_for_string(self) -> Result<String, Error> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err(Error::ValueBadParsing),
        }
    }
    pub fn check_for_float(self) -> Result<f64, Error> {
        match self {
            Value::Float(f) => Ok(f),
            _ => Err(Error::ValueBadParsing),
        }
    }
}

pub(crate) fn parse_integer(buf: &[u8]) -> IResult<&[u8], Value> {
    preceded(space0, map(i64, |val| Value::Integer(val)))(buf)
}

#[cfg(test)]
mod tests {
    use super::{Card, Value};

    #[test]
    fn trimmed_keyword_string() {
        let kw = b"GAIN    ".to_owned();
        let v = Value::Float(30.00);
        let card = Card { kw, v };
        assert_eq!("GAIN".to_owned(), card.keyword().unwrap())
    }
}
