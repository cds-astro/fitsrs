use nom::{
    branch::alt,
    bytes::complete::{take_till, take_while},
    character::complete::{char, space0, i64, hex_digit1},
    combinator::{map, value},
    number::complete::{float},
    sequence::{delimited, preceded},
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

pub(crate) fn white_space0(s: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(|s| s == b' ')(s)
}

pub(crate) fn parse_undefined(buf: &[u8]) -> IResult<&[u8], Value> {
    value(Value::Undefined, white_space0)(buf)
}

pub(crate) fn parse_character_string(buf: &[u8]) -> IResult<&[u8], Value> {
    map(
        preceded(
            space0,
            delimited(char('\''), take_till(|c| c == b'\''), char('\''))
        ),
        |str: &[u8]| {
            // Copy the bytes to a new string
            // This is not a big deal because it only concerns the parsing
            // of the FITS header
            let str = String::from_utf8_lossy(str).into_owned();
            Value::String(str)
        },
    )(buf)
}

pub(crate) fn parse_logical(buf: &[u8]) -> IResult<&[u8], Value> {
    preceded(
        space0,
        alt((
            value(Value::Logical(true), char('T')),
            value(Value::Logical(false), char('F')),
        )),
    )(buf)
}

pub(crate) fn parse_float(buf: &[u8]) -> IResult<&[u8], Value> {
    preceded(
        space0,
        map(float, |val| Value::Float(val as f64))
    )(buf)
}

pub(crate) fn parse_integer(buf: &[u8]) -> IResult<&[u8], Value> {
    preceded(
        space0,
        map(i64, |val| Value::Integer(val)),
    )(buf)
}

#[cfg(test)]
mod tests {
    use super::{parse_character_string, parse_float, Value};
    use crate::card::parse_integer;

    #[test]
    fn test_float() {
        assert_eq!(
            parse_float(b"      -32768.0"),
            Ok((b"" as &[u8], Value::Float(-32768.0)))
        );
        assert_eq!(
            parse_float(b"      -32767"),
            Ok((b"" as &[u8], Value::Float(-32767.0)))
        );
        assert_eq!(
            parse_float(b"      -32767A"),
            Ok((b"A" as &[u8], Value::Float(-32767.0)))
        );
    }
    #[test]
    fn test_string() {
        assert_eq!(
            parse_character_string(b"      'sdfs Zdfs MLKKLSFD sdf '"),
            Ok((
                b"" as &[u8],
                Value::String(String::from("sdfs Zdfs MLKKLSFD sdf "))
            ))
        );
    }
}
