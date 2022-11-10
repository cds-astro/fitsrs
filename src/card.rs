use nom::{
    branch::alt,
    bytes::complete::{take_till, take_while},
    character::complete::{char, space0},
    combinator::{map, value},
    number::complete::float,
    sequence::{delimited, preceded},
    IResult,
};

use serde::Serialize;
#[derive(Debug, PartialEq)]
#[derive(Serialize)]
pub struct Card<'a> {
    kw: Keyword<'a>,
    v: Value<'a>,
}

pub type Keyword<'a> = &'a str;

#[derive(Debug, PartialEq, Clone)]
#[derive(Serialize)]
pub enum Value<'a> {
    IntegerNumber(i64),
    Logical(bool),
    CharacterString(&'a str),
    FloatingPoint(f64),
    Undefined,
}

use crate::error::Error;
impl<'a> Value<'a> {
    pub fn check_for_integer(self) -> Result<i64, Error> {
        match self {
            Value::IntegerNumber(num) => {
                Ok(num)
            }
            _ => Err(Error::ValueBadParsing)
        }
    }
    pub fn check_for_boolean(self) -> Result<bool, Error> {
        match self {
            Value::Logical(logical) => {
                Ok(logical)
            }
            _ => Err(Error::ValueBadParsing)
        }
    }
    pub fn check_for_string(self) -> Result<&'a str, Error> {
        match self {
            Value::CharacterString(s) => {
                Ok(s)
            }
            _ => Err(Error::ValueBadParsing)
        }
    }
    pub fn check_for_float(self) -> Result<f64, Error> {
        match self {
            Value::FloatingPoint(f) => {
                Ok(f)
            }
            _ => Err(Error::ValueBadParsing)
        }
    }
}

pub(crate) fn white_space0(s: &[u8]) -> IResult<&[u8], &[u8]> {
    take_while(|s| s == b' ')(s)
}

pub(crate) fn parse_undefined(buf: &[u8]) -> IResult<&[u8], Value<'_>> {
    value(Value::Undefined, white_space0)(buf)
}

pub(crate) fn parse_character_string(buf: &[u8]) -> IResult<&[u8], Value<'_>> {
    map(
        preceded(
            space0,
            delimited(char('\''), take_till(|c| c == b'\''), char('\'')),
        ),
        |str: &[u8]| {
            let str = std::str::from_utf8(str).unwrap();
            Value::CharacterString(str)
        },
    )(buf)
}

pub(crate) fn parse_logical(buf: &[u8]) -> IResult<&[u8], Value<'_>> {
    preceded(
        space0,
        alt((
            value(Value::Logical(true), char('T')),
            value(Value::Logical(false), char('F')),
        )),
    )(buf)
}

pub(crate) fn parse_float(buf: &[u8]) -> IResult<&[u8], Value<'_>> {
    preceded(
        space0,
        map(float, |val| Value::FloatingPoint(val as f64)),
    )(buf)
}

#[cfg(test)]
mod tests {
    use super::{parse_character_string, parse_float, Value};

    #[test]
    fn test_float() {
        assert_eq!(
            parse_float(b"      -32768.0"),
            Ok((b"" as &[u8], Value::FloatingPoint(-32768.0)))
        );
        assert_eq!(
            parse_float(b"      -32767"),
            Ok((b"" as &[u8], Value::FloatingPoint(-32767.0)))
        );
    }
    #[test]
    fn test_string() {
        assert_eq!(
            parse_character_string(b"      'sdfs Zdfs MLKKLSFD sdf '"),
            Ok((
                b"" as &[u8],
                Value::CharacterString("sdfs Zdfs MLKKLSFD sdf ")
            ))
        );
    }
}
