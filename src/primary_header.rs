use serde::Serialize;
use super::card::{self, Card};

use std::collections::HashMap;

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
pub struct PrimaryHeader<'a> {
    /* Non mandatory keywords */
    cards: HashMap<card::Keyword<'a>, card::Value<'a>>,

    /* Mandatory keywords for fits images parsing */
    // BITPIX: type of the pixel stored in the data block
    bitpix: BitpixValue,
    // NAXIS1, NAXIS2 ,...: size in pixels of each axis
    naxis_size: Vec<usize>,
    // NAXIS: the number of axis
    naxis: usize,
}

use crate::error::Error;
use std::io::Read;
use std::io::Cursor;
fn consume_next_card<T: AsRef<[u8]>>(reader: &mut Cursor<T>, buf: &mut [u8; 80]) -> Result<(), Error> {
    reader.read_exact(buf).map_err(|_| Error::FailReadingNextBytes)?;
    Ok(())
}

fn parse_generic_card<'a>(card: &'a [u8; 80]) -> Result<Option<Card<'a>>, Error> {
    let (card, kw) = parse_card_keyword(card)?;
    let card = if kw != b"END" {
        let (_, v) = parse_card_value(card)?;
        let kw = std::str::from_utf8(kw)?;

        Some(Card { kw, v })
    } else {
        None
    };

    Ok(card)
}

fn check_card_keyword<'a>(card: &'a [u8; 80], keyword: &[u8]) -> Result<card::Value<'a>, Error> {
    if let Some(Card { kw, v }) = parse_generic_card(card)? {
        if kw.as_bytes() == &keyword[..kw.len()] {
            Ok(v)
        } else {
            Err(Error::FailFindingKeyword)
        }
    } else {
        Err(Error::FailFindingKeyword)
    }
}

/* Parse mandatory keywords */
fn parse_bitpix_card(card: &[u8; 80]) -> Result<BitpixValue, Error> {
    match check_card_keyword(card, b"BITPIX")?.check_for_integer()? {
        8 => Ok(BitpixValue::U8),
        16 => Ok(BitpixValue::I16),
        32 => Ok(BitpixValue::I32),
        64 => Ok(BitpixValue::I64),
        -32 => Ok(BitpixValue::F32),
        -64 => Ok(BitpixValue::F64),
        _ => Err(Error::BitpixBadValue),
    }
}
fn parse_naxis_card(card: &[u8; 80]) -> Result<usize, Error> {
    let naxis = check_card_keyword(card, b"NAXIS")?
        .check_for_integer()?;

    Ok(naxis as usize)
}

const NAXIS_KW: [&[u8]; 3] = [b"NAXIS1", b"NAXIS2", b"NAXIS3"];
impl<'a> PrimaryHeader<'a> {
    pub(crate) fn parse<T: AsRef<[u8]> + 'a>(reader: &mut Cursor<T>, bytes_read: &mut usize) -> Result<Self, Error> {
        let mut cards = HashMap::new();

        let mut CARD_BUFFER: [u8; 80] = [b' '; 80];

        // Consume mandatory keywords
        consume_next_card(reader, &mut CARD_BUFFER)?;
        let _ = check_card_keyword(&CARD_BUFFER, b"SIMPLE")?;
        consume_next_card(reader, &mut CARD_BUFFER)?;
        let bitpix = parse_bitpix_card(&CARD_BUFFER)?;

        consume_next_card(reader, &mut CARD_BUFFER)?;
        let naxis = parse_naxis_card(&CARD_BUFFER)?;
    
        let mut naxis_size = vec![];
        for idx in 0..naxis {
            consume_next_card(reader, &mut CARD_BUFFER)?;
            let size = check_card_keyword(&CARD_BUFFER, NAXIS_KW[idx])?
                .check_for_integer()?;
            naxis_size.push(size as usize);
        }
        // Consume until the next non mandatory cards until `END` is reached
        consume_next_card(reader, &mut CARD_BUFFER)?;
        while let Some(card::Card { kw, v }) = parse_generic_card(&CARD_BUFFER)? {
            cards.insert(kw, v);
            consume_next_card(reader, &mut CARD_BUFFER)?;
        }

        /* The last card was a END one */
        Ok(Self {
            cards,

            bitpix,
            naxis,
            naxis_size,
        })
    }

    pub fn get_naxis(&self) -> usize {
        self.naxis
    }

    pub fn get_axis_size(&self, idx: usize) -> Option<&usize> {
        // NAXIS indexes begins at 1 instead of 0
        self.naxis_size.get(idx - 1)
    }

    pub fn get_bitpix(&self) -> &BitpixValue {
        &self.bitpix
    }

    pub fn get(&self, key: &str) -> Option<&card::Value> {
        self.cards.get(key)
    }
}

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
pub enum BitpixValue {
    U8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

type MyResult<'a, I, O> = Result<(I, O), Error>;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_till},
    character::complete::{digit1, multispace0},
    combinator::recognize,
    sequence::{pair, preceded},
    IResult,
};

/*pub(self) fn parse_card(card: &'_ [u8; 80]) -> Result<Card<'_>, Error<'_>> {
    let (card, keyword) = preceded(multispace0, parse_card_keyword)(card.as_slice())?;
    // We stop consuming tokens after the exit
    if keyword == b"END" {
        return Ok(Card::End);
    }

    let (_, value) = parse_card_value(card)?;
    match (keyword, value) {
        // SIMPLE = true check
        (b"SIMPLE", value) => match value {
            CardValue::Logical(true) => Ok(Card::Simple),
            _ => Err(Error::MandatoryValueError("SIMPLE")),
        },
        // BITPIX in {8, 16, 32, 64, -32, -64} check
        (b"BITPIX", value) => match value {
            FITSCardValue::FloatingPoint(bitpix) => {
                match bitpix as i32 {
                    8 => Ok(Card::Bitpix(BitpixValue::U8)),
                    16 => Ok(Card::Bitpix(BitpixValue::I16)),
                    32 => Ok(Card::Bitpix(BitpixValue::I32)),
                    64 => Ok(Card::Bitpix(BitpixValue::I64)),
                    -32 => Ok(Card::Bitpix(BitpixValue::F32)),
                    -64 => Ok(Card::Bitpix(BitpixValue::F64)),
                    _ => Err(Error::BitpixBadValue),
                }
            }
            _ => Err(Error::MandatoryValueError("BITPIX")),
        },
        // NAXIS > 0 integer check
        (b"NAXIS", value) => match value {
            CardValue::FloatingPoint(naxis) => {
                if naxis <= 0.0 {
                    Err(Error::NegativeOrNullNaxis)
                } else {
                    Ok(Card::Naxis(naxis as usize))
                }
            }
            _ => Err(Error::MandatoryValueError("NAXIS")),
        },
        // BLANK value
        (b"BLANK", value) => match value {
            CardValue::FloatingPoint(blank) => Ok(Card::Blank(blank)),
            _ => Err(Error::MandatoryValueError("BLANK")),
        },
        // Comment associated to a string check
        (b"COMMENT", value) => match value {
            CardValue::CharacterString(str) => Ok(Card::Comment(str)),
            _ => Err(Error::MandatoryValueError("COMMENT")),
        },
        ([b'N', b'A', b'X', b'I', b'S', ..], value) => {
            let name = std::str::from_utf8(keyword).unwrap();
            let (_, idx_axis) =
                (preceded(tag(b"NAXIS"), digit1)(keyword) as IResult<&[u8], &[u8]>).unwrap();

            let idx_axis = std::str::from_utf8(idx_axis)
                .map(|str| str.parse::<usize>().unwrap())
                .unwrap();
            if let CardValue::FloatingPoint(size) = value {
                if size <= 0.0 {
                    Err(Error::NegativeOrNullNaxisSize(idx_axis))
                } else {
                    // Check the value
                    Ok(Card::NaxisSize {
                        name,
                        idx: idx_axis,
                        size: size as usize,
                    })
                }
            } else {
                Err(Error::MandatoryValueError(name))
            }
        }
        (keyword, value) => Ok(
            Card::Other {
                name: keyword,
                value,
            }
        ),
    }
}*/

pub(crate) fn parse_card_keyword(buf: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((
        recognize(pair(tag(b"NAXIS"), digit1)),
        take_till(|c| c == b' ' || c == b'\t' || c == b'='),
    ))(buf)
}

use crate::card::*;
pub(crate) fn parse_card_value(buf: &[u8]) -> IResult<&[u8], Value> {
    preceded(
        white_space0,
        alt((
            preceded(
                tag(b"= "),
                alt((parse_character_string, parse_logical, parse_float)),
            ),
            parse_undefined,
        )),
    )(buf)
}

#[cfg(test)]
mod tests {
    use super::{parse_generic_card, Card, Value};
    #[test]
    fn test_parse_card() {
        assert_eq!(
            parse_generic_card(
                b"AZSDFGFC=                    T                                                  "
            ),
            Ok(Some(
                Card {
                    kw: "AZSDFGFC",
                    v: Value::Logical(true)
                }
            ))
        );
        assert_eq!(
            parse_generic_card(
                b"CDS_1=                     T                                                    "
            ),
            Ok(Some(
                Card {
                    kw: "CDS_1",
                    v: Value::Logical(true)
                }
            ))
        );
    }
}
