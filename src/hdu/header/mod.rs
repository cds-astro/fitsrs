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
use std::convert::TryFrom;
use std::io::Read;

use crate::{
    card::{self, *},
    error::Error,
};

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

/*
fn str_to_kw(name: &str) -> Keyword {
    let bytes = name[0..8].as_bytes();
    // 1. Init the fixed keyword slice
    let mut owned_kw: Keyword = [0; 8];
    // 2. Copy from slice
    owned_kw.copy_from_slice(bytes);
    owned_kw
}
*/

fn kw_to_string(name: &Keyword) -> String {
    String::from_utf8_lossy(name.trim_ascii()).into_owned()
}

pub fn check_card_keyword(card: &[u8; 80], keyword: &[u8; 8]) -> Result<card::Value, Error> {
    if card[..8] == keyword[..] {
        if let Card::Value { value, .. } = Card::try_from(card)? {
            Ok(value)
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
    /// All cards as found in the header.
    cards: Vec<Card>,
    /// The value of all cards that represent a value mapped by keyword `name``.
    ///
    /// * A keyword `name` is a trimmed string and not the eight-byte keyword buffer used on the [Card].
    /// * If a keyword appears more than once in the header, the value of the last [Card] is returned
    values: HashMap<String, Value>,
    /// Mandatory keywords for fits ext parsing.
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
        let mut cards = Vec::new();

        /* Consume mandatory keywords */
        let mut xtension: X =
            Xtension::parse(reader, num_bytes_read, card_80_bytes_buf, &mut cards)?;

        /* Consume next non mandatory keywords until `END` is reached */
        loop {
            consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
            if let Ok(card) = Card::try_from(&*card_80_bytes_buf) {
                cards.push(card);
                if Some(&Card::End) == cards.last() {
                    break;
                }
            } else {
                // FIXME log warning
                // preserve the unparsable header
                let card = Card::Undefined(String::from_utf8_lossy(card_80_bytes_buf).into_owned());
                cards.push(card);
            }
        }

        let values = process_cards(&cards)?;
        xtension.update_with_parsed_header(&values)?;
        Ok(Self {
            cards,
            values,
            xtension,
        })
    }

    pub(crate) async fn parse_async<'a, R>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error>
    where
        R: AsyncBufRead + std::marker::Unpin,
    {
        let mut cards = Vec::new();

        /* Consume mandatory keywords */
        let mut xtension: X =
            Xtension::parse_async(reader, num_bytes_read, card_80_bytes_buf, &mut cards).await?;

        loop {
            consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
            if let Ok(card) = Card::try_from(&*card_80_bytes_buf) {
                cards.push(card);
                if Some(&Card::End) == cards.last() {
                    break;
                }
            } else {
                // FIXME log warning
                // preserve the unparsable header
                let card = Card::Undefined(String::from_utf8_lossy(card_80_bytes_buf).into_owned());
                cards.push(card);
            }
        }

        let values = process_cards(&cards)?;
        xtension.update_with_parsed_header(&values)?;
        Ok(Self {
            cards,
            values,
            xtension,
        })
    }

    /// Get the gcount value given by the "PCOUNT" card
    pub fn get_xtension(&self) -> &X {
        &self.xtension
    }

    /// Get the value of a card, returns `None` if the card is not
    /// found or is not a value card.
    pub fn get(&self, key: &[u8; 8]) -> Option<&Value> {
        let kw = String::from_utf8_lossy(key);
        self.values.get(kw.trim())
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
            let value = card.clone();
            <T as CardValue>::parse(value.clone()).map_err(|_| {
                let card = String::from_utf8_lossy(key);
                Error::FailTypeCardParsing(card.to_string(), std::any::type_name::<T>().to_string())
            })
        })
    }

    pub fn keywords(&self) -> Keys<String, Value> {
        self.values.keys()
    }

    pub fn cards(&self) -> impl Iterator<Item = &Card> + use<'_, X> {
        self.cards.iter()
    }

    /// Return an iterator over the processing history of the header, i.e. all
    /// [cards](Card) using the `HISTORY` keyword.
    ///
    pub fn history(&self) -> impl Iterator<Item = &String> + use<'_, X> {
        self.filter_string_values_by_kw("HISTORY")
    }

    /// Return all header level comments, i.e. [cards](Card) using the `COMMENT`
    /// or blank (eight spaces) keyword.
    ///
    /// Note that comments on [Card::Value] cards are part of the [value](Value).
    pub fn comments(&self) -> impl Iterator<Item = &String> + use<'_, X> {
        self.filter_string_values_by_kw("COMMENT")
    }

    fn filter_string_values_by_kw<'a>(
        &'a self,
        filter: &'a str,
    ) -> impl Iterator<Item = &'a String> + use<'a, X> {
        self.cards.iter().filter_map(move |c| {
            if let Card::Value {
                name,
                value: Value::String { value, .. },
            } = c
            {
                if name == filter {
                    return Some(value);
                }
            }
            None
        })
    }
}

fn process_cards(cards: &[Card]) -> Result<HashMap<String, Value>, Error> {
    let mut values = HashMap::new();
    let mut kw: Option<String> = None;

    for (i, card) in cards.iter().enumerate() {
        match card {
            Card::Value { name, value } => {
                if kw.is_some() {
                    return Err(Error::StaticError("expected continuation, found value"));
                }
                values.insert(name.to_owned(), value.to_owned());
                if value.continued() {
                    kw = Some(name.to_owned());
                }
            }
            Card::Continuation { string, comment } => {
                if let Some(ref name) = kw {
                    if let Some(v) = values.get_mut(name) {
                        v.append(string, comment);
                        if !v.continued() {
                            kw = None
                        }
                    } else {
                        unreachable!("algorithm should have added value for continued keyword")
                    }
                } else {
                    return Err(Error::StaticError(
                        "continuation without a continued string value",
                    ));
                }
            }
            Card::Extension(x) => {
                if kw.is_some() {
                    return Err(Error::StaticError("expected continuation, found extension"));
                }
                values.insert(
                    "XTENSION".to_owned(),
                    Value::String {
                        value: String::from_utf8_lossy(x).into_owned(),
                        comment: None,
                    },
                );
            }
            Card::Comment(c) => {
                if kw.is_some() {
                    return Err(Error::StaticError("expected continuation, found comment"));
                }
                values
                    .entry("COMMENT".to_owned())
                    .and_modify(|s| {
                        if let Value::String { value, .. } = s {
                            value.push('\n');
                            value.push_str(c);
                        } else {
                            unreachable!("COMMENT entry must be a Value::String")
                        }
                    })
                    .or_insert(Value::String {
                        value: c.to_owned(),
                        comment: None,
                    });
            }
            Card::History(h) => {
                if kw.is_some() {
                    return Err(Error::StaticError("expected continuation, found history"));
                }
                values
                    .entry("HISTORY".to_owned())
                    .and_modify(|s| {
                        if let Value::String { value, .. } = s {
                            value.push('\n');
                            value.push_str(h);
                        } else {
                            unreachable!("HISTORY entry must be a Value::String")
                        }
                    })
                    .or_insert(Value::String {
                        value: h.to_owned(),
                        comment: None,
                    });
            }
            Card::End => {
                if i + 1 == cards.len() {
                    return Ok(values);
                } else {
                    unreachable!("cards trailing after the END card")
                }
            }
            Card::Space => ( /* NOOP */),
            Card::Undefined(_) => ( /* NOOP */),
        }
    }
    Err(Error::StaticError("Missing END card"))
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

    use crate::fits::Fits;
    use crate::hdu::HDU;

    use core::panic;
    use std::collections::VecDeque;
    use std::fs::File;
    use std::io::Cursor;
    use std::io::Read;

    use super::check_card_keyword;
    use super::Value;

    #[test]
    fn card_keyword() {
        let card = b"STRKEY  = 'Some text'                                                           ";
        if let Ok(Value::String{ value, .. } ) = check_card_keyword(card, b"STRKEY  ") {
            assert_eq!(value, "Some text")
        } else {
            panic!("could not find extension in card")
        }
    }

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
                let mut actuals = hdu.get_header().keywords().collect::<Vec<_>>();
                actuals.sort_unstable();

                let mut expected = VecDeque::from(vec![
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
                ]);

                for actual in actuals {
                    assert_eq!(actual, expected.pop_front().unwrap())
                }
            }
            _ => unreachable!(),
        }
    }
}
