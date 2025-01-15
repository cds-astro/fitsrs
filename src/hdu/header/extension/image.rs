use std::collections::HashMap;
use std::io::Read;

use async_trait::async_trait;
use futures::AsyncRead;
use serde::Serialize;

use crate::card::Card;
use crate::card::Value;
use crate::error::Error;
use crate::hdu::header::consume_next_card_async;
use crate::hdu::header::kw_to_string;
use crate::hdu::header::parse_bitpix_card;
use crate::hdu::header::parse_naxis_card;
use crate::hdu::header::BitpixValue;
use crate::hdu::header::Xtension;
use crate::hdu::header::NAXIS_KW;
use crate::hdu::primary::check_card_keyword;
use crate::hdu::primary::consume_next_card;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Image {
    // A number of bit that each pixel has
    bitpix: BitpixValue,
    // The number of axis
    naxis: usize,
    // The size of each axis
    naxisn: Vec<u64>,
}

impl Image {
    /// Get the number of axis given by the "NAXIS" card
    pub fn get_naxis(&self) -> usize {
        self.naxis
    }

    /// Get the size of an axis given by the "NAXISX" card
    pub fn get_naxisn(&self, idx: usize) -> Option<&u64> {
        // NAXIS indexes begins at 1 instead of 0
        self.naxisn.get(idx - 1)
    }

    /// Get the bitpix value given by the "BITPIX" card
    pub fn get_bitpix(&self) -> BitpixValue {
        self.bitpix
    }
}

#[async_trait(?Send)]
impl Xtension for Image {
    fn get_num_bytes_data_block(&self) -> u64 {
        let num_pixels = if self.naxisn.is_empty() {
            0
        } else {
            self.naxisn.iter().fold(1, |mut total, val| {
                total *= val;
                total
            })
        };

        let num_bits = ((self.bitpix as i32).unsigned_abs() as u64) * num_pixels;
        num_bits >> 3
    }

    fn update_with_parsed_header(&mut self, _cards: &HashMap<String,Value>) -> Result<(), Error> {
        Ok(())
    }

    fn parse<R: Read>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
        cards: &mut Vec<Card>,
    ) -> Result<Self, Error> {
        // BITPIX
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let bitpix = parse_bitpix_card(card_80_bytes_buf)?;
        // NAXIS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis = parse_naxis_card(card_80_bytes_buf)?;
        // The size of each NAXIS
        let naxisn = (0..naxis)
            .map(|idx_axis| {
                consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
                check_card_keyword(card_80_bytes_buf, NAXIS_KW[idx_axis])?
                    .check_for_float()
                    .map(|size| size as u64)
            })
            .collect::<Result<Vec<_>, _>>()?;

        for (i, &naxisi) in naxisn.iter().enumerate() {
            let card = Card::Value {
                name: kw_to_string(NAXIS_KW[i]),
                value: Value::Integer {
                    value: naxisi as i64,
                    comment: None
                }
            };
            cards.push(card);
        }

        let card = Card::Value {
            name: "NAXIS".to_owned(),
            value: Value::Integer {
                value: naxis as i64,
                comment: None
            }
        };
        cards.push(card);

        Ok(Image {
            bitpix,
            naxis,
            naxisn,
        })
    }

    async fn parse_async<R>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
        cards: &mut Vec<Card>,
    ) -> Result<Self, Error>
    where
        R: AsyncRead + std::marker::Unpin,
        Self: Sized,
    {
        // BITPIX
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let bitpix = parse_bitpix_card(card_80_bytes_buf)?;
        // NAXIS
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let naxis = parse_naxis_card(card_80_bytes_buf)?;
        // The size of each NAXIS
        let mut naxisn = vec![];
        for naxis_kw in NAXIS_KW.iter().take(naxis) {
            consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
            let naxis_len = check_card_keyword(card_80_bytes_buf, naxis_kw)?
            .check_for_float()
            .map(|size| size as u64)?;
            // TODO parse comment

            naxisn.push(naxis_len);
        }

        for (i, &naxisi) in naxisn.iter().enumerate() {
            let card = Card::Value {
                name: kw_to_string(NAXIS_KW[i]),
                value: Value::Integer {
                    value: naxisi as i64,
                    comment: None
                }
            };
            cards.push(card);
        }
        let card = Card::Value {
            name: "NAXIS".to_owned(),
            value: Value::Integer {
                value: naxis as i64,
                comment: None
            }
        };
        cards.push(card);

        Ok(Image {
            bitpix,
            naxis,
            naxisn,
        })
    }
}
