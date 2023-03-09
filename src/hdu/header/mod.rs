//! Module implementing the header part of a HDU
//!
//! A header basically consists of a list a 80 long characters CARDS
//! Each CARD is a dictionnary tuple-like of the (key, value) form.
use futures::{AsyncBufRead, AsyncRead, AsyncReadExt};
use serde::Serialize;

pub mod extension;

use std::collections::HashMap;
use std::io::Read;

use crate::card::*;
use crate::card::{self, Card};
use crate::error::Error;

use crate::hdu::Xtension;

use nom::{branch::alt, bytes::complete::tag, sequence::preceded, IResult};

pub fn consume_next_card<R: Read>(
    reader: &mut R,
    buf: &mut [u8; 80],
    bytes_read: &mut u64,
) -> Result<(), Error> {
    *bytes_read += 80;
    reader
        .read_exact(buf)
        .map_err(|_| Error::FailReadingNextBytes)?;

    Ok(())
}

pub async fn consume_next_card_async<'a, R: AsyncRead + std::marker::Unpin>(
    reader: &mut R,
    buf: &mut [u8; 80],
    bytes_read: &mut u64,
) -> Result<(), Error> {
    *bytes_read += 80;
    reader
        .read_exact(buf)
        .await
        .map_err(|_| Error::FailReadingNextBytes)?;
    Ok(())
}

fn parse_generic_card(card: &[u8; 80]) -> Result<Option<Card>, Error> {
    let kw = &card[..8];
    if kw != b"END     " {
        let (_, v) = parse_card_value(&card[8..])?;
        // 1. Init the fixed keyword slice
        let mut owned_kw: [u8; 8] = [0; 8];
        // 2. Copy from slice
        owned_kw.copy_from_slice(kw);

        Ok(Some(Card::new(owned_kw, v)))
    } else {
        Ok(None)
    }
}

pub fn check_card_keyword(card: &[u8; 80], keyword: &[u8; 8]) -> Result<card::Value, Error> {
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
    let naxis = check_card_keyword(card, b"NAXIS   ")?.check_for_float()?;

    Ok(naxis as usize)
}

const NAXIS_KW: [&[u8; 8]; 3] = [b"NAXIS1  ", b"NAXIS2  ", b"NAXIS3  "];

#[derive(Debug, PartialEq, Serialize, Clone, Copy)]
pub enum BitpixValue {
    U8 = 8,
    I16 = 16,
    I32 = 32,
    I64 = 64,
    F32 = -32,
    F64 = -64,
}

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

#[derive(Debug, PartialEq, Serialize)]
pub struct Header<X> {
    /* Non mandatory keywords */
    cards: HashMap<Keyword, Value>,

    /* Mandatory keywords for fits ext parsing */
    xtension: X,
}

impl<X> Header<X>
where
    X: Xtension + std::fmt::Debug,
{
    pub(crate) fn parse<R: Read>(
        reader: &mut R,
        num_bytes_read: &mut u64,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error> {
        let mut cards = HashMap::new();

        /* Consume mandatory keywords */
        let mut xtension: X = Xtension::parse(reader, num_bytes_read, card_80_bytes_buf)?;

        /* Consume next non mandatory keywords until `END` is reached */
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        while let Some(Card { kw, v }) = parse_generic_card(card_80_bytes_buf)? {
            cards.insert(kw, v);
            consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        }

        xtension.update_with_parsed_header(&cards)?;

        /* The last card was a END one */
        Ok(Self { cards, xtension })
    }

    pub(crate) async fn parse_async<'a, R>(
        reader: &mut R,
        num_bytes_read: &mut u64,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error>
    where
        R: AsyncBufRead + std::marker::Unpin,
    {
        let mut cards = HashMap::new();

        /* Consume mandatory keywords */
        let mut xtension: X =
            Xtension::parse_async(reader, num_bytes_read, card_80_bytes_buf).await?;

        /* Consume next non mandatory keywords until `END` is reached */
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        while let Some(Card { kw, v }) = parse_generic_card(card_80_bytes_buf)? {
            cards.insert(kw, v);
            consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        }

        xtension.update_with_parsed_header(&cards)?;

        /* The last card was a END one */
        Ok(Self { cards, xtension })
    }

    /// Get the gcount value given by the "PCOUNT" card
    pub fn get_xtension(&self) -> &X {
        &self.xtension
    }

    /// Get the value of a specific card
    /// # Params
    /// * `key` - The key of a card
    pub fn get(&self, key: &[u8; 8]) -> Option<&Value> {
        self.cards.get(key)
    }

    /// Get the value a specific card and try to parse the value
    /// Returns an error if the asking type does not match the true inner type of
    /// the value got
    /// # Params
    /// * `key` - The key of a card
    pub fn get_parsed<T>(&self, key: &[u8; 8]) -> Option<Result<T, Error>>
    where
        T: CardValue,
    {
        self.get(key)
            .map(|value| <T as CardValue>::parse(value.clone()))
    }
}

fn parse_pcount_card(card: &[u8; 80]) -> Result<usize, Error> {
    let pcount = check_card_keyword(card, b"PCOUNT  ")?.check_for_float()?;

    Ok(pcount as usize)
}

fn parse_gcount_card(card: &[u8; 80]) -> Result<usize, Error> {
    let gcount = check_card_keyword(card, b"GCOUNT  ")?.check_for_float()?;

    Ok(gcount as usize)
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
            Ok(Some(Card {
                kw: b"AZSDFGFC".to_owned(),
                v: Value::Logical(true)
            }))
        );
        assert_eq!(
            parse_generic_card(
                b"CDS_1   =                     T                                                 "
            ),
            Ok(Some(Card {
                kw: b"CDS_1   ".to_owned(),
                v: Value::Logical(true)
            }))
        );
    }
}
