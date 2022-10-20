use std::collections::HashSet;
use serde::Serialize;

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
pub struct PrimaryHeader<'a> {
    #[serde(skip_serializing)]
    pub keys: HashSet<&'a str>,

    // Only serialize the cards
    pub cards: Vec<(&'a str, FITSCard<'a>)>,
}

use crate::error::Error;
impl<'a> PrimaryHeader<'a> {
    pub(crate) fn new(mut input: &'a [u8]) -> MyResult<&'a [u8], Self> {
        let mut cards = Vec::new();
        let mut keys = HashSet::new();

        let mut end = false;

        let mut simple = false;
        let mut naxis = false;
        let mut bitpix = false;
        while !end && !input.is_empty() {
            let (input_next, card) = parse_card(input)?;
            input = input_next;

            println!("card: {:?}", card);

            let key = match card {
                FITSCard::Simple => {
                    simple = true;
                    "SIMPLE"
                }
                FITSCard::Bitpix(_) => {
                    bitpix = true;
                    "BITPIX"
                }
                FITSCard::Naxis(_) => {
                    naxis = true;
                    "NAXIS"
                }
                FITSCard::Blank(_) => "BLANK",
                FITSCard::NaxisSize { name, .. } => name,
                FITSCard::Comment(_) => "COMMENT",
                FITSCard::History(_) => "HISTORY",
                FITSCard::Other { name, .. } => {
                    std::str::from_utf8(name)?
                },
                FITSCard::End => {
                    end = true;
                    continue;
                }
            };

            cards.push((key, card));
            keys.insert(key);
        }
        use std::borrow::Cow;
        // Check mandatory keys are present
        if !simple {
            Err(Error::MandatoryKeywordMissing(Cow::Borrowed("SIMPLE")))
        } else if !bitpix {
            Err(Error::MandatoryKeywordMissing(Cow::Borrowed("BITPIX")))
        } else if !naxis {
            Err(Error::MandatoryKeywordMissing(Cow::Borrowed("NAXIS")))
        } else {
            // Check the NAXISM
            let naxis = &cards.iter().find(|(name, _)| name == &"NAXIS").unwrap().1;

            if let FITSCard::Naxis(naxis) = naxis {
                for idx_axis in 0..*naxis {
                    let key = String::from("NAXIS") + &(idx_axis + 1).to_string();
                    if !keys.contains(&key as &str) {
                        return Err(Error::MandatoryKeywordMissing(key.into()));
                    }
                }

                let header = Self { cards, keys };
                Ok((input, header))
            } else {
                Err(Error::MandatoryKeywordMissing("NAXISM".into()))
            }
        }
    }

    pub(crate) fn get_naxis(&self) -> usize {
        if let Some(&FITSCard::Naxis(naxis)) = self.get("NAXIS") {
            naxis
        } else {
            unreachable!();
        }
    }

    /*pub(crate) fn get_blank(&self) -> f64 {
        if let Some(&FITSHeaderKeyword::Blank(blank)) = self.get("BLANK") {
            blank
        } else {
            unreachable!();
        }
    }*/

    pub(crate) fn get_axis_size(&self, idx: usize) -> Option<usize> {
        // NAXIS indexes begins at 1 instead of 0
        let naxis = String::from("NAXIS") + &(idx + 1).to_string();
        if let Some(FITSCard::NaxisSize { size, .. }) = self.get(&naxis) {
            Some(*size)
        } else {
            None
        }
    }

    pub(crate) fn get_bitpix(&self) -> &BitpixValue {
        if let Some(FITSCard::Bitpix(bitpix)) = self.get("BITPIX") {
            bitpix
        } else {
            unreachable!();
        }
    }

    pub fn get(&self, key: &str) -> Option<&FITSCard> {
        if self.keys.contains(key) {
            let card = &self.cards.iter().find(|card| key == card.0).unwrap().1;

            Some(card)
        } else {
            None
        }
    }
}

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
pub enum FITSCard<'a> {
    Simple,
    Bitpix(BitpixValue),
    Naxis(usize),
    NaxisSize {
        name: &'a str,
        // Index of the axis
        idx: usize,
        // Size of the axis
        size: usize,
    },
    Blank(f64),
    // TODO we will probably need a Cow<str> here
    // because we have to delete simple quote doublons
    Comment(&'a str),
    History(&'a str),
    Other {
        name: &'a [u8],
        value: FITSCardValue<'a>,
    },
    End,
}

use crate::card::FITSCardValue;
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

type MyResult<'a, I, O> = Result<(I, O), Error<'a>>;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_till},
    character::complete::{digit1, multispace0},
    combinator::recognize,
    sequence::{pair, preceded},
    IResult,
};
use nom::bytes::complete::take;
pub(self) fn parse_card(header: &[u8]) -> MyResult<&[u8], FITSCard> {
    // First parse the keyword
    let (bytes, card) = take(80_u8)(header)?;

    let (card, keyword) = preceded(multispace0, parse_card_keyword)(card)?;
    // We stop consuming tokens after the exit
    if keyword == b"END" {
        return Ok((bytes, FITSCard::End));
    }

    let (_, value) = parse_card_value(card)?;
    match (keyword, value) {
        // SIMPLE = true check
        (b"SIMPLE", value) => match value {
            FITSCardValue::Logical(true) => Ok((bytes, FITSCard::Simple)),
            _ => Err(Error::MandatoryValueError("SIMPLE")),
        },
        // BITPIX in {8, 16, 32, 64, -32, -64} check
        (b"BITPIX", value) => match value {
            FITSCardValue::FloatingPoint(bitpix) => {
                match bitpix as i32 {
                    8 => Ok((bytes, FITSCard::Bitpix(BitpixValue::U8))),
                    16 => Ok((bytes, FITSCard::Bitpix(BitpixValue::I16))),
                    32 => Ok((bytes, FITSCard::Bitpix(BitpixValue::I32))),
                    64 => Ok((bytes, FITSCard::Bitpix(BitpixValue::I64))),
                    -32 => Ok((bytes, FITSCard::Bitpix(BitpixValue::F32))),
                    -64 => Ok((bytes, FITSCard::Bitpix(BitpixValue::F64))),
                    _ => Err(Error::BitpixBadValue),
                }
            }
            _ => Err(Error::MandatoryValueError("BITPIX")),
        },
        // NAXIS > 0 integer check
        (b"NAXIS", value) => match value {
            FITSCardValue::FloatingPoint(naxis) => {
                if naxis <= 0.0 {
                    Err(Error::NegativeOrNullNaxis)
                } else {
                    Ok((bytes, FITSCard::Naxis(naxis as usize)))
                }
            }
            _ => Err(Error::MandatoryValueError("NAXIS")),
        },
        // BLANK value
        (b"BLANK", value) => match value {
            FITSCardValue::FloatingPoint(blank) => Ok((bytes, FITSCard::Blank(blank))),
            _ => Err(Error::MandatoryValueError("BLANK")),
        },
        // Comment associated to a string check
        (b"COMMENT", value) => match value {
            FITSCardValue::CharacterString(str) => Ok((bytes, FITSCard::Comment(str))),
            _ => Err(Error::MandatoryValueError("COMMENT")),
        },
        // History associated to a string check
        /*(b"HISTORY", value) => match value {
            FITSKeywordValue::CharacterString(str) => Ok((bytes, FITSHeaderKeyword::History(str))),
            _ => {
                println!("{:?}", value);
                Err(Error::MandatoryValueError("HISTORY"))
            },
        },*/
        ([b'N', b'A', b'X', b'I', b'S', ..], value) => {
            let name = std::str::from_utf8(keyword).unwrap();
            let (_, idx_axis) =
                (preceded(tag(b"NAXIS"), digit1)(keyword) as IResult<&[u8], &[u8]>).unwrap();

            let idx_axis = std::str::from_utf8(idx_axis)
                .map(|str| str.parse::<usize>().unwrap())
                .unwrap();
            if let FITSCardValue::FloatingPoint(size) = value {
                if size <= 0.0 {
                    Err(Error::NegativeOrNullNaxisSize(idx_axis))
                } else {
                    // Check the value
                    Ok((
                        bytes,
                        FITSCard::NaxisSize {
                            name,
                            idx: idx_axis,
                            size: size as usize,
                        },
                    ))
                }
            } else {
                Err(Error::MandatoryValueError(name))
            }
        }
        (keyword, value) => Ok((
            bytes,
            FITSCard::Other {
                name: keyword,
                value,
            },
        )),
    }
}

pub(crate) fn parse_card_keyword(buf: &[u8]) -> IResult<&[u8], &[u8]> {
    alt((
        recognize(pair(tag(b"NAXIS"), digit1)),
        take_till(|c| c == b' ' || c == b'\t' || c == b'='),
    ))(buf)
}

use crate::card::*;
pub(crate) fn parse_card_value(buf: &[u8]) -> IResult<&[u8], FITSCardValue> {
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
    use super::{parse_card, FITSCard, FITSCardValue};
    #[test]
    fn test_parse_card() {
        assert_eq!(
            parse_card(
                b"AZSDFGFC=                    T                                                  "
            ),
            Ok((
                b"" as &[u8],
                FITSCard::Other {
                    name: b"AZSDFGFC",
                    value: FITSCardValue::Logical(true)
                }
            ))
        );
        assert_eq!(
            parse_card(
                b"CDS_1=                     T                                                    "
            ),
            Ok((
                b"" as &[u8],
                FITSCard::Other {
                    name: b"CDS_1",
                    value: FITSCardValue::Logical(true)
                }
            ))
        );
    }
}
