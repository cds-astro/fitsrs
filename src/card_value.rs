use nom::{
    branch::alt,
    character::complete::{alphanumeric0, char, digit1, space0},
    combinator::{map, value},
    sequence::{delimited, preceded, tuple},
    IResult,
};

#[derive(Debug, PartialEq, Clone)]
pub enum FITSKeywordValue<'a> {
    IntegerNumber(i64),
    Logical(bool),
    CharacterString(&'a str),
    FloatingPoint(f64),
    Undefined,
}

pub(crate) fn parse_undefined(buf: &[u8]) -> IResult<&[u8], FITSKeywordValue> {
    value(FITSKeywordValue::Undefined, space0)(buf)
}

pub(crate) fn parse_character_string(buf: &[u8]) -> IResult<&[u8], FITSKeywordValue> {
    map(
        preceded(space0, delimited(char('\''), alphanumeric0, char('\''))),
        |str: &[u8]| {
            let str = std::str::from_utf8(str).unwrap();
            FITSKeywordValue::CharacterString(str)
        },
    )(buf)
}

pub(crate) fn parse_logical(buf: &[u8]) -> IResult<&[u8], FITSKeywordValue> {
    preceded(
        space0,
        alt((
            value(FITSKeywordValue::Logical(true), char('T')),
            value(FITSKeywordValue::Logical(false), char('F')),
        )),
    )(buf)
}

pub(crate) fn parse_integer(buf: &[u8]) -> IResult<&[u8], FITSKeywordValue> {
    preceded(
        space0,
        alt((
            map(
                alt((digit1, preceded(char('+'), digit1))),
                |bytes: &[u8]| {
                    let string = std::str::from_utf8(bytes).unwrap();
                    let value = string.parse::<i64>().unwrap();
                    FITSKeywordValue::IntegerNumber(value)
                },
            ),
            map(preceded(char('-'), digit1), |bytes: &[u8]| {
                let string = std::str::from_utf8(bytes).unwrap();
                let value = string.parse::<i64>().unwrap();
                FITSKeywordValue::IntegerNumber(-value)
            }),
        )),
    )(buf)
}

pub(crate) fn parse_absolute_float(buf: &[u8]) -> IResult<&[u8], f64> {
    map(
        tuple((digit1, char('.'), digit1)),
        |(digits, _, decimals): (&[u8], char, &[u8])| {
            let string = std::format!(
                "{}.{}",
                std::str::from_utf8(digits).unwrap(),
                std::str::from_utf8(decimals).unwrap()
            );

            string.parse::<f64>().unwrap()
        },
    )(buf)
}

pub(crate) fn parse_float(buf: &[u8]) -> IResult<&[u8], FITSKeywordValue> {
    preceded(
        space0,
        alt((
            map(
                alt((
                    parse_absolute_float,
                    preceded(char('+'), parse_absolute_float),
                )),
                |value| {
                    FITSKeywordValue::FloatingPoint(value)
                },
            ),
            map(preceded(char('-'), parse_absolute_float), |value: f64| {
                FITSKeywordValue::FloatingPoint(-value)
            }),
        )),
    )(buf)
}

#[cfg(test)]
mod tests {
    use super::{parse_integer, FITSKeywordValue};

    #[test]
    fn test_integer() {
        assert_eq!(
            parse_integer(b"      -4545424"),
            Ok((b"" as &[u8], FITSKeywordValue::IntegerNumber(-4545424)))
        );
    }
}
