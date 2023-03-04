use std::io::Read;
use std::collections::HashMap;

use serde::Serialize;

use crate::hdu::Xtension;
use crate::hdu::primary::consume_next_card;
use crate::error::Error;
use crate::hdu::primary::check_card_keyword;
use crate::hdu::header::parse_pcount_card;
use crate::hdu::header::parse_gcount_card;
use crate::hdu::header::NAXIS_KW;
use crate::hdu::header::parse_naxis_card;
use crate::hdu::header::parse_bitpix_card;
use crate::hdu::header::BitpixValue;
use crate::card::Value;

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
#[derive(Clone)]
pub struct AsciiTable {
    // Should be 1
    bitpix: BitpixValue,
    // Number of axis, Should be 2,
    naxis: usize,
    // A non-negative integer, giving the number of ASCII characters in each row of
    // the table. This includes all the characters in the defined fields
    // plus any characters that are not included in any field.
    naxis1: usize,
    // A non-negative integer, giving the number of rows in the table
    naxis2: usize,
    // A non-negative integer representing the number of fields in each row.
    // The maximum permissible value is 999.
    tfields: usize,
    // Integers specifying the column in which Field n starts.
    // The first column of a row is numbered 1.
    tbcols: Vec<usize>,
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
    pub fn get_naxis1(&self) -> usize {
        self.naxis1
    }

    #[inline]
    pub fn get_naxis2(&self) -> usize {
        self.naxis1
    }

    #[inline]
    pub fn get_tfields(&self) -> usize {
        self.tfields
    }

    #[inline]
    pub fn get_tbcols(&self) -> &[usize] {
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

impl Xtension for AsciiTable {
    fn get_num_bytes_data_block(&self) -> usize {
        let num_ascii_char = self.naxis1 * self.naxis2;
        num_ascii_char
    }

    fn update_with_parsed_header(&mut self, cards: &HashMap<[u8; 8], Value>) -> Result<(), Error> {
        // TBCOLS
        self.tbcols = (0..self.tfields)
            .map(|idx_field| {
                let idx_field = idx_field + 1;
                let kw = format!("TBCOL{:?}       ", idx_field);
                let kw_bytes = kw.as_bytes();

                // 1. Init the fixed keyword slice
                let mut owned_kw: [u8; 8] = [0; 8];
                // 2. Copy from slice
                owned_kw.copy_from_slice(&kw_bytes[..8]);

                cards.get(&owned_kw)
                    .ok_or(Error::StaticError("TBCOLX card not found"))?
                    .clone()
                    .check_for_float()
                    .map(|tbcol| tbcol as usize)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // TFORMS
        self.tforms = (0..self.tfields)
            .map(|idx_field| {
                let idx_field = idx_field + 1;
                let kw = format!("TFORM{:?}       ", idx_field);
                let kw_bytes = kw.as_bytes();

                // 1. Init the fixed keyword slice
                let mut owned_kw: [u8; 8] = [0; 8];
                // 2. Copy from slice
                owned_kw.copy_from_slice(&kw_bytes[..8]);

                let tform = dbg!(cards.get(&owned_kw)
                    .ok_or(Error::StaticError("TFORMX card not found"))?
                    .clone()
                    .check_for_string()?);



                let first_char = &tform[0..1];
                match first_char {
                    "A" => {
                        let w = tform[1..].trim_end().parse::<i32>()
                            .map_err(|_| Error::StaticError("expected w after the "))?;

                        Ok(TFormAsciiTable::Character { w: w as usize })
                    },
                    "I" => {
                        let w = tform[1..].trim_end().parse::<i32>()
                            .map_err(|_| Error::StaticError("expected w after the "))?;

                        Ok(TFormAsciiTable::DecimalInteger { w: w as usize })
                    },
                    "F" => {
                        let wd = tform[1..].trim_end().split(".").collect::<Vec<_>>();

                        let w = dbg!(wd[0]
                            .parse::<i32>()
                            .map_err(|_| Error::StaticError("TFORM E type: w part does not parse into an integer"))?);

                        let d = wd[1]
                            .parse::<i32>()
                            .map_err(|_| Error::StaticError("TFORM E type: d part does not parse into an integer"))?;

                        Ok(TFormAsciiTable::FloatingPointFixed { w: w as usize, d: d as usize })
                    },
                    "E" => {
                        let wd = tform[1..].trim_end().split(".").collect::<Vec<_>>();

                        let w = dbg!(wd[0]
                            .parse::<i32>()
                            .map_err(|_| Error::StaticError("TFORM E type: w part does not parse into an integer"))?);

                        let d = wd[1]
                            .parse::<i32>()
                            .map_err(|_| Error::StaticError("TFORM E type: d part does not parse into an integer"))?;

                        Ok(TFormAsciiTable::EFloatingPointExp { w: dbg!(w as usize), d: d as usize })
                    },
                    "D" => {
                        let wd = tform[1..].trim_end().split(".").collect::<Vec<_>>();

                        let w = dbg!(wd[0]
                            .parse::<i32>()
                            .map_err(|_| Error::StaticError("TFORM E type: w part does not parse into an integer"))?);

                        let d = dbg!(wd[1]
                            .parse::<i32>()
                            .map_err(|_| Error::StaticError("TFORM E type: d part does not parse into an integer"))?);

                        Ok(TFormAsciiTable::DFloatingPointExp { w: dbg!(w) as usize, d: dbg!(d) as usize })
                    },
                    _ => Err(Error::StaticError("Ascii Table TFORM not recognized"))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    fn parse<R: Read>(reader: &mut R, num_bytes_read: &mut usize, card_80_bytes_buf: &mut [u8; 80]) -> Result<Self, Error> {
        // BITPIX
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let bitpix = parse_bitpix_card(&card_80_bytes_buf)?;
        if bitpix != BitpixValue::U8 {
            return Err(Error::StaticError("Ascii Table HDU must have a BITPIX = 8"));
        }

        // NAXIS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis = parse_naxis_card(&card_80_bytes_buf)?;

        if naxis != 2 {
            return Err(Error::StaticError("Ascii Table HDU must have NAXIS = 2"));
        }
        // NAXIS1
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis1 = check_card_keyword(&card_80_bytes_buf, NAXIS_KW[0])?
            .check_for_float()? as usize;
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis2 = check_card_keyword(&card_80_bytes_buf, NAXIS_KW[1])?
            .check_for_float()? as usize;

        // PCOUNT
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let pcount = parse_pcount_card(&card_80_bytes_buf)?;
        if pcount != 0 {
            return Err(Error::StaticError("Ascii Table HDU must have PCOUNT = 0"));
        }

        // GCOUNT
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let gcount = parse_gcount_card(&card_80_bytes_buf)?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let tfields = check_card_keyword(&card_80_bytes_buf, b"TFIELDS ")?
            .check_for_float()? as usize;

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
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub enum TFormAsciiTable {
    Character {
        w: usize,
    },
    DecimalInteger {
        w: usize,
    },
    FloatingPointFixed {
        w: usize,
        d: usize,
    },
    EFloatingPointExp {
        w: usize,
        d: usize,
    },
    DFloatingPointExp {
        w: usize,
        d: usize,
    },
}