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
    pub fn get(&self, key: &Keyword) -> Option<&Value> {
        let kw = String::from_utf8_lossy(key);
        self.values.get(kw.trim())
    }

    /// Get the value for a value card, returns `None` if the keyword is not
    /// found or if the card is not a value card.
    pub fn value(&self, keyword: &str) -> Option<&Value> {
        self.values.get(keyword)
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
        self.cards.iter().filter_map(move |c|
            if let Card::History(string) = c {
                Some(string)
            } else {
                None
            }
        )
    }

    /// Return all header level comments, i.e. [cards](Card) using the `COMMENT`
    /// or blank (eight spaces) keyword.
    ///
    /// Note that comments on [Card::Value] cards are part of the [value](Value).
    pub fn comments(&self) -> impl Iterator<Item = &String> + use<'_, X> {
        self.cards.iter().filter_map(move |c|
            if let Card::Comment(string) = c {
                Some(string)
            } else {
                None
            }
        )
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
            Card::Xtension{ x, .. } => {
                if kw.is_some() {
                    return Err(Error::StaticError("expected continuation, found extension"));
                }
                values.insert(
                    "XTENSION".to_owned(),
                    Value::String {
                        value: (*x).into(),
                        comment: None,
                    },
                );
            }
            Card::End => {
                if i + 1 == cards.len() {
                    return Ok(values);
                } else {
                    unreachable!("cards trailing after the END card")
                }
            }
            _ => ( /* NOOP - TODO log debug */),
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

    use crate::card::Card;
    use crate::error::Error;
    use crate::fits::Fits;
    use crate::hdu::HDU;

    use core::panic;
    use std::collections::VecDeque;
    use std::fs::File;

    use std::io::Cursor;
    use std::io::Read;

    use super::check_card_keyword;
    // use Iterator;

    use std::iter::Iterator;
    use super::CardBuf;

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
    fn primary_hdu_with_no_data() -> Result<(), Error> {
        let data = mock_fits_data([
            b"SIMPLE  =                    T / this is a fake FITS file                       ",
            b"BITPIX  =                    8 / byte sized numbers                             ",
            b"NAXIS   =                    0 / no data arrays                                 ",
            b"COMMENT some contextual comment on the header                                   ",
            b"COMMENT ... over two lines                                                      ",
            b"HISTORY this was processed manually using vscode                                ",
            b"COMMENT comment on the history?                                                 ",
            b"HISTORY did some more processing...                                             ",
            b"END                                                                             "
        ]);
        let reader = Cursor::new(data);
        let mut fits = Fits::from_reader(reader);
        let hdu = fits
            .next()
            .expect("Should contain a primary HDU")
            .unwrap()
            ;
        assert!(matches!(hdu, HDU::Primary(_)));
        if let HDU::Primary(hdu) = hdu {
            let mut cards = hdu.get_header().cards();
            // FIXME ensure that SIMPLE is added to header cards
            // assert_eq!(cards.next(), Some(&Card::Value {
            //     name: "SIMPLE".to_owned(),
            //     value: Value::Logical {
            //         value: true,
            //         comment: Some(" this is a FITS file".to_owned())
            //     }
            // }));
            // FIXME ensure that BITPIX is added to header cards
            // assert_eq!(cards.next(), Some(&Card::Value {
            //     name: "BITPIX".to_owned(),
            //     value: Value::Integer {
            //         value: 8,
            //         comment: Some(" byte sized numbers".to_owned())
            //     }
            // }));
            assert_eq!(cards.next(), Some(&Card::Value {
                name: "NAXIS".to_owned(),
                value: Value::Integer {
                    value: 0,
                    // FIXME ensure that the a NAXIS comment is preserved
                    // comment: Some(" no data arrays".to_owned())
                    comment: None
                }
            }));
            assert_eq!(cards.next(), Some(&Card::Comment("some contextual comment on the header".to_owned())));
            assert_eq!(cards.next(), Some(&Card::Comment("... over two lines".to_owned())));
            assert_eq!(cards.next(), Some(&Card::History("this was processed manually using vscode".to_owned())));
            assert_eq!(cards.next(), Some(&Card::Comment("comment on the history?".to_owned())));
            assert_eq!(cards.next(), Some(&Card::History("did some more processing...".to_owned())));
            assert_eq!(cards.next(), Some(&Card::End));
            assert_eq!(cards.next(), None);

            let header = hdu.get_header();

            let mut comments = header.comments();
            assert_eq!(comments.next(), Some(&"some contextual comment on the header".to_string()));
            assert_eq!(comments.next(), Some(&"... over two lines".to_string()));
            assert_eq!(comments.next(), Some(&"comment on the history?".to_string()));
            assert_eq!(comments.next(), None);

            let mut history = header.history();
            assert_eq!(history.next(), Some(&"this was processed manually using vscode".to_string()));
            assert_eq!(history.next(), Some(&"did some more processing...".to_string()));
            assert_eq!(history.next(), None);

        }

        Ok(())
    }

    /// panics if N > 36
    fn mock_fits_data<const N: usize>(cards: [&CardBuf; N]) -> [u8; 2880] {
        let mut data = [b' '; 2880];
        let mut cursor = 0;
        for card in cards {
            data[cursor..cursor+80].copy_from_slice(card);
            cursor += 80;
        }
        data
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
                    "CD2_2", "CDELT1", "CDELT2", "CDELTM1", "CDELTM2", /* "COMMENT",*/ "COMPRESS",
                    "CRPIX1", "CRPIX2", "CRVAL1", "CRVAL2", "CTYPE1", "CTYPE2", "CUNIT1",
                    "CUNIT2", "CVF", "DATAMAX", "DATAMIN", "DATE", "DATE-OBS", "DEC",
                    "DEWPOINT", "DIAMETER", "ERRFLUX", "EXPOSURE", "FILTERS", "FOCAL",
                    "FOCUSPOS", "FOCUSTMP", "GAIN_ELE", /* "HISTORY",*/ "HUMIDITY", "IMGTYPE",
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
