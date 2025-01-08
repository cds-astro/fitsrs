//! Module implementing the header part of a HDU
//!
//! A header basically consists of a list a 80 long characters CARDS
//! Each CARD is a dictionnary tuple-like of the (key, value) form.
use futures::{AsyncBufRead, AsyncRead, AsyncReadExt};
use serde::Serialize;

pub mod extension;

pub use extension::Xtension;

use std::collections::hash_map::Keys;
use std::collections::HashMap;
use std::io::Read;

use crate::card::*;
use crate::card::{self, Card};
use crate::error::Error;

pub fn consume_next_card<R: Read>(
    reader: &mut R,
    buf: &mut [u8; 80],
    bytes_read: &mut usize,
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
    bytes_read: &mut usize,
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
    let value = match kw {
        b"END     " => None,
        b"HISTORY " => {
            let v = Value::String(String::from_utf8_lossy(&card[8..]).to_string());
            Some(v)
        }
        _ => {
            let str = String::from_utf8_lossy(&card[8..]);
            // Take until the comment beginning with '/'
            let str = str.split('/').next().unwrap();
            let v = if str.is_empty() {
                // empty value (just a comment present)
                Value::Undefined
            } else {
                // not empty value, there is at least one character
                // check if it is an =
                let str = if str.chars().next().unwrap() == '=' {
                    &str[1..]
                } else {
                    &str
                };

                // remove the ' ' before and after
                let str = str.trim();

                if str.is_empty() {
                    Value::Undefined
                } else if let Ok(val) = str.parse::<f64>() {
                    Value::Float(val)
                } else if let Ok(val) = str.parse::<i64>() {
                    Value::Integer(val)
                } else if str == "T" {
                    Value::Logical(true)
                } else if str == "F" {
                    Value::Logical(false)
                } else {
                    // Last case check for a string
                    let inside_str = str.split('\'').collect::<Vec<_>>();

                    if inside_str.len() >= 2 {
                        // This is a true string because it is nested inside simple quotes
                        Value::String(inside_str[1].to_string())
                    } else {
                        // This is not a string but we did not attempt to parse it
                        // so we store it as a string value
                        Value::String(str.to_string())
                    }
                }
            };
            Some(v)
        }
    };

    // TODO parse comment
    let comment = None;

    if let Some(value) = value {
        // 1. Init the fixed keyword slice
        let mut owned_kw: [u8; 8] = [0; 8];
        // 2. Copy from slice
        owned_kw.copy_from_slice(kw);

        Ok(Some(Card::new(owned_kw, value, comment)))
    } else {
        Ok(None)
    }
}

pub fn check_card_keyword(card: &[u8; 80], keyword: &[u8; 8]) -> Result<card::Value, Error> {
    if let Some(Card { kw, v, c: None }) = parse_generic_card(card)? {
        if &kw == keyword {
            Ok(v)
        } else {
            Err(Error::FailFindingKeyword(
                std::str::from_utf8(keyword)?.to_owned(),
            ))
        }
    } else {
        Err(Error::FailFindingKeyword(
            std::str::from_utf8(keyword)?.to_owned(),
        ))
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

const NAXIS_KW: [&[u8; 8]; 6] = [
    b"NAXIS1  ",
    b"NAXIS2  ",
    b"NAXIS3  ",
    b"NAXIS4  ",
    b"NAXIS5  ",
    b"NAXIS6  ",
];

#[derive(Debug, PartialEq, Serialize, Clone, Copy)]
pub enum BitpixValue {
    U8 = 8,
    I16 = 16,
    I32 = 32,
    I64 = 64,
    F32 = -32,
    F64 = -64,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Header<X> {
    /* Keywords */
    cards: HashMap<Keyword, Card>,

    /* Mandatory keywords for fits ext parsing */
    xtension: X,
}

impl<X> Header<X>
where
    X: Xtension + std::fmt::Debug,
{
    pub(crate) fn parse<R: Read>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error> {
        let mut cards = HashMap::new();

        /* Consume mandatory keywords */
        let mut xtension: X =
            Xtension::parse(reader, num_bytes_read, card_80_bytes_buf, &mut cards)?;

        /* Consume next non mandatory keywords until `END` is reached */
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        while let Some(card) = parse_generic_card(card_80_bytes_buf)? {
            let kw = card.kw.clone();
            cards.insert(kw, card);
            consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        }

        xtension.update_with_parsed_header(&cards)?;

        /* The last card was a END one */
        Ok(Self { cards, xtension })
    }

    pub(crate) async fn parse_async<'a, R>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error>
    where
        R: AsyncBufRead + std::marker::Unpin,
    {
        let mut cards = HashMap::new();

        /* Consume mandatory keywords */
        let mut xtension: X =
            Xtension::parse_async(reader, num_bytes_read, card_80_bytes_buf, &mut cards).await?;

        /* Consume next non mandatory keywords until `END` is reached */
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        while let Some(card) = parse_generic_card(card_80_bytes_buf)? {
            cards.insert(card.kw.clone(), card);
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
    pub fn get(&self, key: &[u8; 8]) -> Option<&Card> {
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
        self.get(key).map(|card| {
            let value = card.v.clone();
            <T as CardValue>::parse(value.clone()).map_err(|_| {
                let card = String::from_utf8_lossy(key);
                Error::FailTypeCardParsing(card.to_string(), std::any::type_name::<T>().to_string())
            })
        })
    }

    pub fn keywords(&self) -> Keys<Keyword, Card> {
        self.cards.keys()
    }

    pub fn cards(&self) -> impl Iterator<Item = Card> + use<'_, X> {
        self.cards.iter().map(|(kw, card)| Card {
            kw: kw.to_owned(),
            v: card.v.to_owned(),
            c: card.c.to_owned(),
        })
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
                v: Value::Logical(true),
                c: None,
            }))
        );
        assert_eq!(
            parse_generic_card(
                b"CDS_1   =                     T                                                 "
            ),
            Ok(Some(Card {
                kw: b"CDS_1   ".to_owned(),
                v: Value::Logical(true),
                c: None,
            }))
        );
    }

    use crate::fits::Fits;
    use crate::hdu::HDU;

    use std::fs::File;
    use std::io::Cursor;
    use std::io::Read;
    #[test]
    fn test_keywords_iter() {
        let f = File::open("samples/misc/SN2923fxjA.fits").unwrap();
        let bytes: Result<Vec<_>, _> = f.bytes().collect();
        let buf = bytes.unwrap();

        let reader = Cursor::new(&buf[..]);
        let mut hdu_list = Fits::from_reader(reader);

        let hdu = hdu_list.next().unwrap().unwrap();
        match hdu {
            HDU::Primary(hdu) => {
                let mut keywords = hdu
                    .get_header()
                    .cards()
                    .map(|c| c.keyword().map(|k| k.to_owned()))
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                keywords.sort_unstable();

                assert_eq!(
                    keywords,
                    vec![
                        "BAYERIND", "BITCAMPX", "CALPHOT", "CCD-TEMP", "CD1_1", "CD1_2", "CD2_1",
                        "CD2_2", "CDELT1", "CDELT2", "CDELTM1", "CDELTM2", "COMMENT", "COMPRESS",
                        "CRPIX1", "CRPIX2", "CRVAL1", "CRVAL2", "CTYPE1", "CTYPE2", "CUNIT1",
                        "CUNIT2", "CVF", "DATAMAX", "DATAMIN", "DATE", "DATE-OBS", "DEC",
                        "DEWPOINT", "DIAMETER", "ERRFLUX", "EXPOSURE", "FILTERS", "FOCAL",
                        "FOCUSPOS", "FOCUSTMP", "GAIN_ELE", "HISTORY", "HUMIDITY", "IMGTYPE",
                        "INSTRUME", "MAGREF", "MIRORX", "NAXIS", "NAXIS1", "NAXIS2", "OBJCTDEC",
                        "OBJCTRA", "OBSERVER", "OFFSET_E", "ORIGIN", "P3DSPHER", "PCXASTRO",
                        "PCYASTRO", "PDEC_REF", "PDIMPOL", "PIERSIDE", "PPLATESD", "PPXC", "PPYC",
                        "PRADIUSX", "PRADIUSY", "PRA_REF", "PRESSURE", "PSOLDC1", "PSOLDC10",
                        "PSOLDC11", "PSOLDC12", "PSOLDC13", "PSOLDC14", "PSOLDC15", "PSOLDC16",
                        "PSOLDC17", "PSOLDC18", "PSOLDC19", "PSOLDC2", "PSOLDC20", "PSOLDC21",
                        "PSOLDC22", "PSOLDC23", "PSOLDC24", "PSOLDC25", "PSOLDC26", "PSOLDC27",
                        "PSOLDC28", "PSOLDC29", "PSOLDC3", "PSOLDC30", "PSOLDC31", "PSOLDC32",
                        "PSOLDC33", "PSOLDC34", "PSOLDC35", "PSOLDC36", "PSOLDC4", "PSOLDC5",
                        "PSOLDC6", "PSOLDC7", "PSOLDC8", "PSOLDC9", "PSOLRA1", "PSOLRA10",
                        "PSOLRA11", "PSOLRA12", "PSOLRA13", "PSOLRA14", "PSOLRA15", "PSOLRA16",
                        "PSOLRA17", "PSOLRA18", "PSOLRA19", "PSOLRA2", "PSOLRA20", "PSOLRA21",
                        "PSOLRA22", "PSOLRA23", "PSOLRA24", "PSOLRA25", "PSOLRA26", "PSOLRA27",
                        "PSOLRA28", "PSOLRA29", "PSOLRA3", "PSOLRA30", "PSOLRA31", "PSOLRA32",
                        "PSOLRA33", "PSOLRA34", "PSOLRA35", "PSOLRA36", "PSOLRA4", "PSOLRA5",
                        "PSOLRA6", "PSOLRA7", "PSOLRA8", "PSOLRA9", "PSOLX1", "PSOLX10", "PSOLX11",
                        "PSOLX12", "PSOLX13", "PSOLX14", "PSOLX15", "PSOLX16", "PSOLX17",
                        "PSOLX18", "PSOLX19", "PSOLX2", "PSOLX20", "PSOLX21", "PSOLX22", "PSOLX23",
                        "PSOLX24", "PSOLX25", "PSOLX26", "PSOLX27", "PSOLX28", "PSOLX29", "PSOLX3",
                        "PSOLX30", "PSOLX31", "PSOLX32", "PSOLX33", "PSOLX34", "PSOLX35",
                        "PSOLX36", "PSOLX4", "PSOLX5", "PSOLX6", "PSOLX7", "PSOLX8", "PSOLX9",
                        "PSOLY1", "PSOLY10", "PSOLY11", "PSOLY12", "PSOLY13", "PSOLY14", "PSOLY15",
                        "PSOLY16", "PSOLY17", "PSOLY18", "PSOLY19", "PSOLY2", "PSOLY20", "PSOLY21",
                        "PSOLY22", "PSOLY23", "PSOLY24", "PSOLY25", "PSOLY26", "PSOLY27",
                        "PSOLY28", "PSOLY29", "PSOLY3", "PSOLY30", "PSOLY31", "PSOLY32", "PSOLY33",
                        "PSOLY34", "PSOLY35", "PSOLY36", "PSOLY4", "PSOLY5", "PSOLY6", "PSOLY7",
                        "PSOLY8", "PSOLY9", "PSSX", "PSSY", "RA", "READOUTT", "REFFLUX", "SITELAT",
                        "SITELONG", "STACKNB", "STARCNT", "SWCREATE", "TELESCOP", "TEMPEXT", "UT",
                        "WINDIR", "WINSPEED", "X1", "X2", "XPIXELSZ", "XPIXSZ", "Y1", "Y2",
                        "YPIXELSZ", "YPIXSZ"
                    ]
                );
            }
            _ => unreachable!(),
        }
    }
}
