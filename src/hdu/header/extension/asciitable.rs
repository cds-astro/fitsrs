use std::collections::HashMap;
use std::io::Read;

use async_trait::async_trait;
use futures::AsyncRead;
use serde::Serialize;

use crate::card::Value;
use crate::error::Error;
use crate::hdu::header::consume_next_card_async;
use crate::hdu::header::parse_bitpix_card;
use crate::hdu::header::parse_gcount_card;
use crate::hdu::header::parse_naxis_card;
use crate::hdu::header::parse_pcount_card;
use crate::hdu::header::BitpixValue;
use crate::hdu::header::NAXIS_KW;
use crate::hdu::primary::check_card_keyword;
use crate::hdu::primary::consume_next_card;
use crate::hdu::Xtension;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct AsciiTable {
    // Should be 1
    bitpix: BitpixValue,
    // Number of axis, Should be 2,
    naxis: usize,
    // A non-negative integer, giving the number of ASCII characters in each row of
    // the table. This includes all the characters in the defined fields
    // plus any characters that are not included in any field.
    naxis1: u64,
    // A non-negative integer, giving the number of rows in the table
    naxis2: u64,
    // A non-negative integer representing the number of fields in each row.
    // The maximum permissible value is 999.
    tfields: usize,
    // Integers specifying the column in which Field n starts.
    // The first column of a row is numbered 1.
    tbcols: Vec<u64>,
    // Contain a character string describing the format in which Field n is encoded.
    // Only the formats in Table 15, interpreted as Fortran (ISO 2004)
    // input formats and discussed in more detail in Sect. 7.2.5, are
    // permitted for encoding
    tforms: Vec<TFormAsciiTable>,
    // Should be 0
    pcount: usize,
    // Should be 1
    gcount: usize,
}

impl AsciiTable {
    /// Get the bitpix value given by the "BITPIX" card
    #[inline]
    pub fn get_bitpix(&self) -> BitpixValue {
        self.bitpix
    }

    /// Get the number of axis given by the "NAXIS" card
    #[inline]
    pub fn get_naxis(&self) -> usize {
        self.naxis
    }

    /// Get the size of an axis given by the "NAXISX" card
    #[inline]
    pub fn get_naxis1(&self) -> u64 {
        self.naxis1
    }

    #[inline]
    pub fn get_naxis2(&self) -> u64 {
        self.naxis1
    }

    #[inline]
    pub fn get_tfields(&self) -> usize {
        self.tfields
    }

    #[inline]
    pub fn get_tbcols(&self) -> &[u64] {
        &self.tbcols
    }

    #[inline]
    pub fn get_tforms(&self) -> &[TFormAsciiTable] {
        &self.tforms
    }

    /// Get the pcount value given by the "PCOUNT" card
    #[inline]
    pub fn get_pcount(&self) -> usize {
        self.pcount
    }

    /// Get the gcount value given by the "PCOUNT" card
    #[inline]
    pub fn get_gcount(&self) -> usize {
        self.gcount
    }
}

#[async_trait(?Send)]
impl Xtension for AsciiTable {
    fn get_num_bytes_data_block(&self) -> u64 {
        self.naxis1 * self.naxis2
    }

    fn update_with_parsed_header(&mut self, cards: &HashMap<[u8; 8], Value>) -> Result<(), Error> {
        // TBCOLS
        self.tbcols = (0..self.tfields)
            .map(|idx_field| {
                let idx_field = idx_field + 1;
                let kw = format!("TBCOL{idx_field:?}       ");
                let kw_bytes = kw.as_bytes();

                // 1. Init the fixed keyword slice
                let mut owned_kw: [u8; 8] = [0; 8];
                // 2. Copy from slice
                owned_kw.copy_from_slice(&kw_bytes[..8]);

                cards
                    .get(&owned_kw)
                    .ok_or(Error::StaticError("TBCOLX card not found"))?
                    .clone()
                    .check_for_integer()
                    .map(|tbcol| tbcol as u64)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // TFORMS
        self.tforms = (0..self.tfields)
            .map(|idx_field| {
                let idx_field = idx_field + 1;
                let kw = format!("TFORM{idx_field:?}       ");
                let kw_bytes = kw.as_bytes();

                // 1. Init the fixed keyword slice
                let mut owned_kw: [u8; 8] = [0; 8];
                // 2. Copy from slice
                owned_kw.copy_from_slice(&kw_bytes[..8]);

                let tform = cards
                    .get(&owned_kw)
                    .ok_or(Error::StaticError("TFORMX card not found"))?
                    .clone()
                    .check_for_string()?;

                let first_char = &tform[0..1];
                match first_char {
                    "A" => {
                        let w = tform[1..]
                            .trim_end()
                            .parse::<i32>()
                            .map_err(|_| Error::StaticError("expected w after the "))?;

                        Ok(TFormAsciiTable::Character { w: w as usize })
                    }
                    "I" => {
                        let w = tform[1..]
                            .trim_end()
                            .parse::<i32>()
                            .map_err(|_| Error::StaticError("expected w after the "))?;

                        Ok(TFormAsciiTable::DecimalInteger { w: w as usize })
                    }
                    "F" => {
                        let wd = tform[1..].trim_end().split('.').collect::<Vec<_>>();

                        let w = wd[0].parse::<i32>().map_err(|_| {
                            Error::StaticError(
                                "TFORM E type: w part does not parse into an integer",
                            )
                        })?;

                        let d = wd[1].parse::<i32>().map_err(|_| {
                            Error::StaticError(
                                "TFORM E type: d part does not parse into an integer",
                            )
                        })?;

                        Ok(TFormAsciiTable::FloatingPointFixed {
                            w: w as usize,
                            d: d as usize,
                        })
                    }
                    "E" => {
                        let wd = tform[1..].trim_end().split('.').collect::<Vec<_>>();

                        let w = wd[0].parse::<i32>().map_err(|_| {
                            Error::StaticError(
                                "TFORM E type: w part does not parse into an integer",
                            )
                        })?;

                        let d = wd[1].parse::<i32>().map_err(|_| {
                            Error::StaticError(
                                "TFORM E type: d part does not parse into an integer",
                            )
                        })?;

                        Ok(TFormAsciiTable::EFloatingPointExp {
                            w: w as usize,
                            d: d as usize,
                        })
                    }
                    "D" => {
                        let wd = tform[1..].trim_end().split('.').collect::<Vec<_>>();

                        let w = wd[0].parse::<i32>().map_err(|_| {
                            Error::StaticError(
                                "TFORM E type: w part does not parse into an integer",
                            )
                        })?;

                        let d = wd[1].parse::<i32>().map_err(|_| {
                            Error::StaticError(
                                "TFORM E type: d part does not parse into an integer",
                            )
                        })?;

                        Ok(TFormAsciiTable::DFloatingPointExp {
                            w: w as usize,
                            d: d as usize,
                        })
                    }
                    _ => Err(Error::StaticError("Ascii Table TFORM not recognized")),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    fn parse<R: Read>(
        reader: &mut R,
        num_bytes_read: &mut u64,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error> {
        // BITPIX
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let bitpix = parse_bitpix_card(card_80_bytes_buf)?;
        if bitpix != BitpixValue::U8 {
            return Err(Error::StaticError("Ascii Table HDU must have a BITPIX = 8"));
        }

        // NAXIS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis = parse_naxis_card(card_80_bytes_buf)?;

        if naxis != 2 {
            return Err(Error::StaticError("Ascii Table HDU must have NAXIS = 2"));
        }
        // NAXIS1
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis1 =
            check_card_keyword(card_80_bytes_buf, NAXIS_KW[0])?.check_for_integer()? as u64;
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis2 =
            check_card_keyword(card_80_bytes_buf, NAXIS_KW[1])?.check_for_integer()? as u64;

        // PCOUNT
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let pcount = parse_pcount_card(card_80_bytes_buf)?;
        if pcount != 0 {
            return Err(Error::StaticError("Ascii Table HDU must have PCOUNT = 0"));
        }

        // GCOUNT
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let gcount = parse_gcount_card(card_80_bytes_buf)?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let tfields =
            check_card_keyword(card_80_bytes_buf, b"TFIELDS ")?.check_for_integer()? as usize;

        let tbcols = vec![];
        let tforms = vec![];

        Ok(AsciiTable {
            bitpix,
            naxis,
            naxis1,
            naxis2,
            tbcols,
            tfields,
            tforms,
            pcount,
            gcount,
        })
    }

    async fn parse_async<R>(
        reader: &mut R,
        num_bytes_read: &mut u64,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error>
    where
        R: AsyncRead + std::marker::Unpin,
        Self: Sized,
    {
        // BITPIX
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let bitpix = parse_bitpix_card(card_80_bytes_buf)?;
        if bitpix != BitpixValue::U8 {
            return Err(Error::StaticError("Ascii Table HDU must have a BITPIX = 8"));
        }

        // NAXIS
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let naxis = parse_naxis_card(card_80_bytes_buf)?;

        if naxis != 2 {
            return Err(Error::StaticError("Ascii Table HDU must have NAXIS = 2"));
        }
        // NAXIS1
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let naxis1 =
            check_card_keyword(card_80_bytes_buf, NAXIS_KW[0])?.check_for_integer()? as u64;
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let naxis2 =
            check_card_keyword(card_80_bytes_buf, NAXIS_KW[1])?.check_for_integer()? as u64;

        // PCOUNT
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let pcount = parse_pcount_card(card_80_bytes_buf)?;
        if pcount != 0 {
            return Err(Error::StaticError("Ascii Table HDU must have PCOUNT = 0"));
        }

        // GCOUNT
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let gcount = parse_gcount_card(card_80_bytes_buf)?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let tfields =
            check_card_keyword(card_80_bytes_buf, b"TFIELDS ")?.check_for_integer()? as usize;

        let tbcols = vec![];
        let tforms = vec![];

        Ok(AsciiTable {
            bitpix,
            naxis,
            naxis1,
            naxis2,
            tbcols,
            tfields,
            tforms,
            pcount,
            gcount,
        })
    }
}

// Field value | Data type
// Aw          | Character
// Iw          | Decimal integer
// Fw.d        | Floating-point, fixed decimal notation
// Ew.d        | Floating-point, exponential notation
// Dw.d        | Floating-point, exponential notation
// Notes. w is the width in characters of the field and d is the number of
// digits to the right of the decimal.
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub enum TFormAsciiTable {
    Character { w: usize },
    DecimalInteger { w: usize },
    FloatingPointFixed { w: usize, d: usize },
    EFloatingPointExp { w: usize, d: usize },
    DFloatingPointExp { w: usize, d: usize },
}

#[cfg(test)]
mod tests {
    use super::{AsciiTable, TFormAsciiTable};
    use crate::{
        fits::Fits,
        hdu::{extension::XtensionHDU, header::BitpixValue},
    };
    use std::{fs::File, io::BufReader};

    fn compare_ascii_ext(filename: &str, ascii_table: AsciiTable) {
        let f = File::open(filename).unwrap();

        let mut reader = BufReader::new(f);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        // Get the first HDU extension,
        // this should be the table for these fits examples
        let hdu = hdu
            .next()
            .expect("Should contain an extension HDU")
            .unwrap();
        match hdu {
            XtensionHDU::AsciiTable(hdu) => {
                let xtension = hdu.get_header().get_xtension();
                assert_eq!(xtension.clone(), ascii_table);
            }
            _ => panic!("Should contain a ASCII table HDU extension"),
        }
    }

    // These tests have been manually created thanks to this command on the fits files:
    // strings  samples/fits.gsfc.nasa.gov/HST_HRS.fits | fold -80 | grep "TBCOL" | tr -s ' ' | cut -d ' ' -f 3
    #[test]
    fn test_asciitable_extension() {
        compare_ascii_ext(
            "samples/fits.gsfc.nasa.gov/HST_FGS.fits",
            AsciiTable {
                bitpix: BitpixValue::U8,
                naxis: 2,
                naxis1: 99,
                naxis2: 7,
                tfields: 6,
                tbcols: vec![1, 17, 33, 49, 65, 91],
                tforms: vec![
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 17 },
                    TFormAsciiTable::Character { w: 8 },
                ],
                // Should be 0
                pcount: 0,
                // Should be 1
                gcount: 1,
            },
        );

        compare_ascii_ext(
            "samples/fits.gsfc.nasa.gov/HST_FOC.fits",
            AsciiTable {
                bitpix: BitpixValue::U8,
                naxis: 2,
                naxis1: 312,
                naxis2: 1,
                tfields: 18,
                tbcols: vec![
                    1, 29, 57, 73, 89, 105, 121, 137, 153, 169, 185, 193, 209, 221, 233, 261, 289,
                    301,
                ],
                tforms: vec![
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 16 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 16 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::Character { w: 4 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 16 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 16 },
                    TFormAsciiTable::Character { w: 8 },
                    TFormAsciiTable::Character { w: 8 },
                ],
                // Should be 0
                pcount: 0,
                // Should be 1
                gcount: 1,
            },
        );

        compare_ascii_ext(
            "samples/fits.gsfc.nasa.gov/HST_HRS.fits",
            AsciiTable {
                bitpix: BitpixValue::U8,
                naxis: 2,
                naxis1: 412,
                naxis2: 4,
                tfields: 25,
                tbcols: vec![
                    1, 29, 45, 61, 77, 93, 121, 149, 161, 173, 201, 213, 225, 237, 249, 261, 277,
                    293, 309, 325, 337, 349, 365, 381, 397,
                ],
                tforms: vec![
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 16 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 16 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 16 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 16 },
                    TFormAsciiTable::Character { w: 8 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::DecimalInteger { w: 11 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                ],
                // Should be 0
                pcount: 0,
                // Should be 1
                gcount: 1,
            },
        );

        compare_ascii_ext(
            "samples/fits.gsfc.nasa.gov/HST_WFPC_II.fits",
            AsciiTable {
                bitpix: BitpixValue::U8,
                naxis: 2,
                naxis1: 796,
                naxis2: 4,
                tfields: 49,
                tbcols: vec![
                    1, 27, 53, 69, 85, 101, 117, 133, 149, 165, 181, 183, 199, 212, 225, 251, 277,
                    286, 295, 308, 324, 340, 356, 372, 388, 404, 417, 430, 443, 456, 469, 482, 495,
                    508, 557, 573, 589, 605, 621, 637, 653, 669, 685, 701, 717, 733, 749, 765, 781,
                ],
                tforms: vec![
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 17 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 17 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::Character { w: 1 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 17 },
                    TFormAsciiTable::DFloatingPointExp { w: 25, d: 17 },
                    TFormAsciiTable::Character { w: 8 },
                    TFormAsciiTable::Character { w: 8 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::DecimalInteger { w: 12 },
                    TFormAsciiTable::Character { w: 48 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                    TFormAsciiTable::EFloatingPointExp { w: 15, d: 7 },
                ],
                // Should be 0
                pcount: 0,
                // Should be 1
                gcount: 1,
            },
        );
    }
}
