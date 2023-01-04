//! Module implementing the header part of a HDU
//! 
//! A header basically consists of a list a 80 long characters CARDS
//! Each CARD is a dictionnary tuple-like of the (key, value) form.
use futures::{AsyncBufRead, AsyncReadExt};
use serde::Serialize;
use crate::card::{self, Card};

use std::collections::HashMap;
use std::io::BufRead;

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
pub struct Header {
    /* Non mandatory keywords */
    cards: HashMap<card::Keyword, card::Value>,

    /* Mandatory keywords for fits images parsing */
    // BITPIX: type of the pixel stored in the data block
    bitpix: BitpixValue,
    // NAXIS1, NAXIS2 ,...: size in pixels of each axis
    naxis_size: Vec<usize>,
    // NAXIS: the number of axis
    naxis: usize,
}

use crate::error::Error;
fn consume_next_card<'a, R: BufRead>(reader: &mut R, buf: &mut [u8; 80], bytes_read: &mut usize) -> Result<(), Error> {
    *bytes_read += 80;
    reader.read_exact(buf).map_err(|_| Error::FailReadingNextBytes)?;
    Ok(())
}

async fn consume_next_card_async<'a, R: AsyncBufRead + std::marker::Unpin>(reader: &mut R, buf: &mut [u8; 80], bytes_read: &mut usize) -> Result<(), Error> {
    *bytes_read += 80;
    reader.read_exact(buf).await.map_err(|_| Error::FailReadingNextBytes)?;
    Ok(())
}

fn parse_generic_card(card: &[u8; 80]) -> Result<Option<Card>, Error> {
    let kw = &card[..8];
    let card = if kw != b"END     " {
        let (_, v) = parse_card_value(&card[8..])?;
        // 1. Init the fixed keyword slice
        let mut owned_kw: [u8; 8] = [0; 8];
        // 2. Copy from slice
        owned_kw.copy_from_slice(&kw);

        Some(Card::new(owned_kw, v))
    } else {
        None
    };

    Ok(card)
}

fn check_card_keyword(card: &[u8; 80], keyword: &[u8; 8]) -> Result<card::Value, Error> {
    if let Some(Card { kw, v }) = parse_generic_card(card)? {
        if &kw == keyword {
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
    let bitpix = check_card_keyword(card, b"BITPIX  ")?.check_for_float()? as i32;
    match bitpix {
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
    let naxis = check_card_keyword(card, b"NAXIS   ")?
        .check_for_float()?;

    Ok(naxis as usize)
}

const NAXIS_KW: [&[u8; 8]; 3] = [b"NAXIS1  ", b"NAXIS2  ", b"NAXIS3  "];
use super::data::DataRead;
impl Header {
    pub(crate) fn parse<'a, R: BufRead>(reader: &mut R, bytes_read: &mut usize) -> Result<Self, Error> {
        let mut cards = HashMap::new();

        let mut card_80_bytes_buf: [u8; 80] = [b' '; 80];

        /* Consume mandatory keywords */ 
        // SIMPLE
        consume_next_card(reader, &mut card_80_bytes_buf, bytes_read)?;
        let _ = check_card_keyword(&card_80_bytes_buf, b"SIMPLE  ")?;
        // BITPIX
        consume_next_card(reader, &mut card_80_bytes_buf, bytes_read)?;
        let bitpix = parse_bitpix_card(&card_80_bytes_buf)?;
        // NAXIS
        consume_next_card(reader, &mut card_80_bytes_buf, bytes_read)?;
        let naxis = parse_naxis_card(&card_80_bytes_buf)?;
        // The size of each NAXIS
        let naxis_size: Result<Vec<usize>, _> = (0..naxis)
            .map(|idx_axis| {
                consume_next_card(reader, &mut card_80_bytes_buf, bytes_read)?;
                check_card_keyword(&card_80_bytes_buf, NAXIS_KW[idx_axis])?
                    .check_for_float()
                    .map(|size| size as usize)
            })
            .collect();

        /* Consume next non mandatory keywords until `END` is reached */
        consume_next_card(reader, &mut card_80_bytes_buf, bytes_read)?;
        while let Some(card::Card { kw, v }) = parse_generic_card(&card_80_bytes_buf)? {
            cards.insert(kw, v);
            consume_next_card(reader, &mut card_80_bytes_buf, bytes_read)?;
        }

        /* The last card was a END one */
        Ok(Self {
            cards,

            bitpix,
            naxis,
            naxis_size: naxis_size?,
        })
    }

    pub(crate) async fn parse_async<'a, R: AsyncBufRead + std::marker::Unpin>(reader: &mut R, bytes_read: &mut usize) -> Result<Self, Error> {
        let mut cards = HashMap::new();

        let mut card_80_bytes_buf: [u8; 80] = [b' '; 80];

        /* Consume mandatory keywords */ 
        // SIMPLE
        consume_next_card_async(reader, &mut card_80_bytes_buf, bytes_read).await?;
        let _ = check_card_keyword(&card_80_bytes_buf, b"SIMPLE  ")?;
        // BITPIX
        consume_next_card_async(reader, &mut card_80_bytes_buf, bytes_read).await?;
        let bitpix = parse_bitpix_card(&card_80_bytes_buf)?;
        // NAXIS
        consume_next_card_async(reader, &mut card_80_bytes_buf, bytes_read).await?;
        let naxis = parse_naxis_card(&card_80_bytes_buf)?;
        // The size of each NAXIS
        let mut naxis_size = vec![0; naxis];
        for idx_axis in 0..naxis {
            consume_next_card_async(reader, &mut card_80_bytes_buf, bytes_read).await?;
            let naxis_len = check_card_keyword(&card_80_bytes_buf, NAXIS_KW[idx_axis])?
                .check_for_float()
                .map(|size| size as usize)?;
            naxis_size[idx_axis] = naxis_len;
        }

        /* Consume next non mandatory keywords until `END` is reached */
        consume_next_card_async(reader, &mut card_80_bytes_buf, bytes_read).await?;
        while let Some(card::Card { kw, v }) = parse_generic_card(&card_80_bytes_buf)? {
            cards.insert(kw, v);
            consume_next_card_async(reader, &mut card_80_bytes_buf, bytes_read).await?;
        }

        /* The last card was a END one */
        Ok(Self {
            cards,

            bitpix,
            naxis,
            naxis_size,
        })
    }

    /// Get the number of axis given by the "NAXIS" card
    pub fn get_naxis(&self) -> usize {
        self.naxis
    }

    /// Get the size of an axis given by the "NAXISX" card
    pub fn get_axis_size(&self, idx: usize) -> Option<&usize> {
        // NAXIS indexes begins at 1 instead of 0
        self.naxis_size.get(idx - 1)
    }

    /// Get the bitpix value given by the "BITPIX" card
    pub fn get_bitpix(&self) -> BitpixValue {
        self.bitpix
    }

    /// Get the value of a specific card
    /// # Params
    /// * `key` - The key of a card 
    pub fn get(&self, key: &[u8; 8]) -> Option<&card::Value> {
        self.cards.get(key)
    }

    /// Get the value a specific card and try to parse the value
    /// Returns an error if the asking type does not match the true inner type of
    /// the value got
    /// # Params
    /// * `key` - The key of a card 
    pub fn get_parsed<T>(&self, key: &[u8; 8]) -> Option<Result<T, Error>>
    where
        T: CardValue
    {
        self.get(key)
            .map(|value| <T as CardValue>::parse(value.clone()))
    }
}

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
#[derive(Clone, Copy)]
pub enum BitpixValue {
    U8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

use nom::{
    branch::alt,
    bytes::complete::{tag, take_till},
    character::complete::digit1,
    combinator::recognize,
    sequence::{pair, preceded},
    IResult,
};

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
                    kw: b"AZSDFGFC".to_owned(),
                    v: Value::Logical(true)
                }
            ))
        );
        assert_eq!(
            parse_generic_card(
                b"CDS_1   =                     T                                                 "
            ),
            Ok(Some(
                Card {
                    kw: b"CDS_1   ".to_owned(),
                    v: Value::Logical(true)
                }
            ))
        );
    }
}
