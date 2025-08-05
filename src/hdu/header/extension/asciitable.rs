use async_trait::async_trait;

use log::warn;
use serde::Serialize;

use crate::error::Error;

use crate::hdu::header::Bitpix;

use crate::hdu::header::ValueMap;
use crate::hdu::header::Xtension;
use serde::Deserialize;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct AsciiTable {
    // Should be 1
    bitpix: Bitpix,
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

    /// Get the size of an axis given by the "NAXISX" card
    #[inline]
    pub fn get_naxis1(&self) -> u64 {
        self.naxis1
    }

    #[inline]
    pub fn get_naxis2(&self) -> u64 {
        self.naxis2
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

    pub fn get_num_cols(&self) -> usize {
        self.get_tfields()
    }

    pub fn get_num_rows(&self) -> usize {
        self.get_naxis2() as usize
    }
}

#[async_trait(?Send)]
impl Xtension for AsciiTable {
    fn get_num_bytes_data_block(&self) -> u64 {
        self.naxis1 * self.naxis2
    }

    fn parse(values: &ValueMap) -> Result<Self, Error> {
        // BITPIX
        let bitpix = values.check_for_bitpix()?;
        if bitpix != Bitpix::U8 {
            return Err(Error::StaticError("Ascii Table HDU must have a BITPIX = 8"));
        }

        // NAXIS
        let naxis = values.check_for_naxis()?;
        if naxis != 2 {
            return Err(Error::StaticError("Ascii Table HDU must have NAXIS = 2"));
        }

        // NAXIS1
        let naxis1 = values.check_for_naxisi(1)?;
        let naxis2 = values.check_for_naxisi(2)?;

        // PCOUNT
        let pcount = values.check_for_pcount()?;
        if pcount != 0 {
            return Err(Error::StaticError("Ascii Table HDU must have PCOUNT = 0"));
        }

        // GCOUNT
        let gcount = values.check_for_gcount()?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        let tfields = values.check_for_tfields()?;

        // TFORMS
        let mut tbcols = Vec::with_capacity(tfields);
        let mut tforms = Vec::with_capacity(tfields);
        for idx_field in 1..=tfields {
            let tbcol = match values.get_parsed(&format!("TBCOL{idx_field}")) {
                Ok(tbcol) => tbcol,
                Err(err) => {
                    warn!("Discard field {idx_field}: {err}");
                    continue;
                }
            };

            let tform = match values.get_parsed(&format!("TFORM{idx_field}")) {
                Ok(tform) => tform,
                Err(err) => {
                    warn!("Discard field {idx_field}: {err}");
                    continue;
                }
            };

            tbcols.push(tbcol);
            tforms.push(tform);
        }

        Ok(AsciiTable {
            bitpix,
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

impl<'de> Deserialize<'de> for TFormAsciiTable {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = TFormAsciiTable;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid TFORM string")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let mut chars = v.trim_end().chars();
                let first_char = chars.next().ok_or(E::custom("TFORM string is empty"))?;
                let rest = chars.as_str();

                let parse = |s: &str| {
                    s.parse().map_err(|err| {
                        E::custom(format_args!(
                            "Failed to parse an integer from the TFORM string: {err}"
                        ))
                    })
                };

                let parse_split = || {
                    let (w, d) = rest
                        .split_once('.')
                        .ok_or(E::custom("Expected to find `.` in the TFORM string"))?;

                    Ok::<_, E>((parse(w)?, parse(d)?))
                };

                Ok(match first_char {
                    'A' => {
                        let w = parse(rest)?;

                        TFormAsciiTable::Character { w }
                    }
                    'I' => {
                        let w = parse(rest)?;

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

                        TFormAsciiTable::DFloatingPointExp { w, d }
                    }
                    _ => {
                        return Err(E::custom(format_args!(
                            "Invalid TFORM prefix '{first_char}'"
                        )))
                    }
                })
            }
        }

        deserializer.deserialize_str(Visitor)
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
