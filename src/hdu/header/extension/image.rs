use std::io::Read;

use serde::Serialize;

use crate::hdu::Xtension;
use crate::hdu::primary::consume_next_card;
use crate::error::Error;
use crate::hdu::primary::check_card_keyword;
use crate::hdu::header::NAXIS_KW;
use crate::hdu::header::parse_naxis_card;
use crate::hdu::header::parse_bitpix_card;
use crate::hdu::header::BitpixValue;

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
#[derive(Clone)]
pub struct Image {
    // A number of bit that each pixel has
    bitpix: BitpixValue,
    // The number of axis
    naxis: usize, 
    // The size of each axis
    naxisn: Vec<usize>,
}

impl Image {
    /// Get the number of axis given by the "NAXIS" card
    pub fn get_naxis(&self) -> usize {
        self.naxis
    }

    /// Get the size of an axis given by the "NAXISX" card
    pub fn get_naxisn(&self, idx: usize) -> Option<&usize> {
        // NAXIS indexes begins at 1 instead of 0
        self.naxisn.get(idx - 1)
    }

    /// Get the bitpix value given by the "BITPIX" card
    pub fn get_bitpix(&self) -> BitpixValue {
        self.bitpix
    }
}

impl Xtension for Image {
    fn get_num_bytes_data_block(&self) -> usize {
        let num_pixels = if self.naxisn.is_empty() {
            0
        } else {
            self.naxisn
                .iter()
                .fold(1, |mut total, val| {
                    total *= val;
                    total
                })
        };

        let num_bits = ((self.bitpix as i32).abs() as usize) * num_pixels;
        num_bits >> 3
    }

    fn parse<R: Read>(reader: &mut R, num_bytes_read: &mut usize, card_80_bytes_buf: &mut [u8; 80]) -> Result<Self, Error> {
        // BITPIX
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let bitpix = parse_bitpix_card(&card_80_bytes_buf)?;
        // NAXIS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis = parse_naxis_card(&card_80_bytes_buf)?;
        // The size of each NAXIS
        let naxisn = (0..naxis)
            .map(|idx_axis| {
                consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
                check_card_keyword(&card_80_bytes_buf, NAXIS_KW[idx_axis])?
                    .check_for_float()
                    .map(|size| size as usize)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Image {
            bitpix,
            naxis,
            naxisn
        })
    }
}

