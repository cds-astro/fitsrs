use std::collections::HashMap;

use async_trait::async_trait;

use log::warn;
use serde::Serialize;

use crate::card::Value;
use crate::error::Error;

use crate::hdu::header::check_for_bitpix;
use crate::hdu::header::check_for_gcount;
use crate::hdu::header::check_for_naxis;
use crate::hdu::header::check_for_naxisi;
use crate::hdu::header::check_for_pcount;
use crate::hdu::header::check_for_tfields;
use crate::hdu::header::Bitpix;

use crate::hdu::header::Xtension;
use std::num::ParseIntError;
use std::str::FromStr;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct AsciiTable {
    // Should be 1
    bitpix: Bitpix,
    // Number of axis, Should be 2,
    naxis: u64,
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
    pcount: u64,
    // Should be 1
    gcount: u64,
}

impl AsciiTable {
    /// Get the bitpix value given by the "BITPIX" card
    #[inline]
    pub fn get_bitpix(&self) -> Bitpix {
        self.bitpix
    }

    /// Get the number of axis given by the "NAXIS" card
    #[inline]
    pub fn get_naxis(&self) -> u64 {
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
    pub fn get_pcount(&self) -> u64 {
        self.pcount
    }

    /// Get the gcount value given by the "PCOUNT" card
    #[inline]
    pub fn get_gcount(&self) -> u64 {
        self.gcount
    }
}

#[async_trait(?Send)]
impl Xtension for AsciiTable {
    fn get_num_bytes_data_block(&self) -> u64 {
        self.naxis1 * self.naxis2
    }

    fn parse(values: &HashMap<String, Value>) -> Result<Self, Error> {
        // BITPIX
        let bitpix = check_for_bitpix(values)?;
        if bitpix != Bitpix::U8 {
            return Err(Error::StaticError("Ascii Table HDU must have a BITPIX = 8"));
        }

        // NAXIS
        let naxis = check_for_naxis(values)?;
        if naxis != 2 {
            return Err(Error::StaticError("Ascii Table HDU must have NAXIS = 2"));
        }

        // NAXIS1
        let naxis1 = check_for_naxisi(values, 1)?;
        let naxis2 = check_for_naxisi(values, 2)?;

        // PCOUNT
        let pcount = check_for_pcount(values)?;
        if pcount != 0 {
            return Err(Error::StaticError("Ascii Table HDU must have PCOUNT = 0"));
        }

        // GCOUNT
        let gcount = check_for_gcount(values)?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        let tfields = check_for_tfields(values)?;

        // TFORMS
        let (tbcols, tforms) = (1..=tfields)
            .filter_map(|idx_field| {
                let tbcol = if let Some(Value::Integer { value, .. }) =
                    values.get(&format!("TBCOL{idx_field}"))
                {
                    *value as u64
                } else {
                    warn!("Discard field {idx_field}");
                    return None;
                };

                let tform = if let Some(Value::String { value, .. }) =
                    values.get(&format!("TFORM{idx_field}"))
                {
                    TFormAsciiTable::from_str(value).ok()?
                } else {
                    warn!("Discard field {idx_field}");
                    return None;
                };

                Some((tbcol, tform))
            })
            .unzip();

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

#[derive(Debug)]
pub enum TFormAsciiTableParseError {
    ParseInt(ParseIntError),
    StringFormat,
}

impl From<ParseIntError> for TFormAsciiTableParseError {
    fn from(err: ParseIntError) -> Self {
        TFormAsciiTableParseError::ParseInt(err)
    }
}

impl FromStr for TFormAsciiTable {
    type Err = TFormAsciiTableParseError;

    fn from_str(tform: &str) -> Result<Self, Self::Err> {
        let mut chars = tform.trim_end().chars();
        let first_char = chars
            .next()
            .ok_or(TFormAsciiTableParseError::StringFormat)?;
        let rest = chars.as_str();

        let parse_split = || {
            let (w, d) = rest
                .split_once('.')
                .ok_or(TFormAsciiTableParseError::StringFormat)?;

            let w = w.parse()?;
            let d = d.parse()?;

            Ok::<_, Self::Err>((w, d))
        };

        Ok(match first_char {
            'A' => {
                let w = rest.parse()?;

                TFormAsciiTable::Character { w }
            }
            'I' => {
                let w = rest.parse()?;

                TFormAsciiTable::DecimalInteger { w }
            }
            'F' => {
                let (w, d) = parse_split()?;

                TFormAsciiTable::FloatingPointFixed { w, d }
            }
            'E' => {
                let (w, d) = parse_split()?;

                TFormAsciiTable::EFloatingPointExp { w, d }
            }
            'D' => {
                let (w, d) = parse_split()?;

                TFormAsciiTable::EFloatingPointExp { w, d }
            }
            _ => return Err(TFormAsciiTableParseError::StringFormat),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AsciiTable, TFormAsciiTable};
    use crate::{
        fits::Fits,
        hdu::{header::Bitpix, HDU},
    };
    use std::{fs::File, io::BufReader};

    fn compare_ascii_ext(filename: &str, ascii_table: AsciiTable) {
        let f = File::open(filename).unwrap();

        let reader = BufReader::new(f);
        let mut hdu_list = Fits::from_reader(reader);

        // Get the first HDU extension,
        // this should be the table for these fits examples
        let hdu = hdu_list
            // skip the primary hdu
            .nth(1)
            .expect("Should contain an extension HDU")
            .unwrap();
        match hdu {
            HDU::XASCIITable(hdu) => {
                let xtension = hdu.get_header().get_xtension();
                assert_eq!(xtension.clone(), ascii_table);
            }
            _ => panic!("Should contain a ASCII table HDU extension"),
        }
    }

    // These tests have been manually created thanks to this command on the fits files:
    // strings  samples/fits.gsfc.nasa.gov/HST_HRS.fits | fold -80 | grep "TBCOL" | tr -s ' ' | cut -d ' ' -f 3
    #[test]
    fn test_fits_asciitable_extension() {
        compare_ascii_ext(
            "samples/fits.gsfc.nasa.gov/HST_FGS.fits",
            AsciiTable {
                bitpix: Bitpix::U8,
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
                bitpix: Bitpix::U8,
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
                bitpix: Bitpix::U8,
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
                bitpix: Bitpix::U8,
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
