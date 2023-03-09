use std::collections::HashMap;
use std::fmt::Debug;
use std::io::Read;

use async_trait::async_trait;
use futures::AsyncRead;
use nom::AsChar;
use serde::Serialize;

use crate::card::parse_integer;
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

use crate::card::Value;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct BinTable {
    bitpix: BitpixValue,
    // Number of axis, Should be 2,
    naxis: usize,
    // A non-negative integer, giving the number of eight-bit bytes in each row of the
    // table.
    naxis1: u64,
    // A non-negative integer, giving the number of rows in the table
    naxis2: u64,
    // A non-negative integer representing the number of fields in each row.
    // The maximum permissible value is 999.
    tfields: usize,
    // Contain a character string describing the format in which Field n is encoded.
    // Only the formats in Table 15, interpreted as Fortran (ISO 2004)
    // input formats and discussed in more detail in Sect. 7.2.5, are
    // permitted for encoding
    tforms: Vec<TFormBinaryTableType>,

    pcount: usize,
    // Should be 1
    gcount: usize,
}

#[async_trait(?Send)]
impl Xtension for BinTable {
    fn get_num_bytes_data_block(&self) -> u64 {
        self.naxis1 * self.naxis2
    }

    fn update_with_parsed_header(&mut self, cards: &HashMap<[u8; 8], Value>) -> Result<(), Error> {
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

                let card_value = cards
                    .get(&owned_kw)
                    .ok_or(Error::StaticError("TFIELDS idx does not map any TFORM!"))?
                    .clone()
                    .check_for_string()?;

                let (field_type_char, repeat_count) =
                    if let Ok((remaining_bytes, Value::Float(repeat_count))) =
                        parse_integer(card_value.as_bytes())
                    {
                        (remaining_bytes[0].as_char(), repeat_count)
                    } else {
                        (owned_kw[0].as_char(), 1.0)
                    };
                let repeat_count = repeat_count as u64;

                match field_type_char.as_char() {
                    'L' => Ok(TFormBinaryTableType::L(TFormBinaryTable::new(repeat_count))), // Logical
                    'X' => Ok(TFormBinaryTableType::X(TFormBinaryTable::new(repeat_count))), // Bit
                    'B' => Ok(TFormBinaryTableType::B(TFormBinaryTable::new(repeat_count))), // Unsigned Byte
                    'I' => Ok(TFormBinaryTableType::I(TFormBinaryTable::new(repeat_count))), // 16-bit integer
                    'J' => Ok(TFormBinaryTableType::J(TFormBinaryTable::new(repeat_count))), // 32-bit integer
                    'K' => Ok(TFormBinaryTableType::K(TFormBinaryTable::new(repeat_count))), // 64-bit integer
                    'A' => Ok(TFormBinaryTableType::A(TFormBinaryTable::new(repeat_count))), // Character
                    'E' => Ok(TFormBinaryTableType::E(TFormBinaryTable::new(repeat_count))), // Single-precision floating point
                    'D' => Ok(TFormBinaryTableType::D(TFormBinaryTable::new(repeat_count))), // Double-precision floating point
                    'C' => Ok(TFormBinaryTableType::C(TFormBinaryTable::new(repeat_count))), // Single-precision complex
                    'M' => Ok(TFormBinaryTableType::M(TFormBinaryTable::new(repeat_count))), // Double-precision complex
                    'P' => Ok(TFormBinaryTableType::P(TFormBinaryTable::new(repeat_count))), // Array Descriptor (32-bit)
                    'Q' => Ok(TFormBinaryTableType::Q(TFormBinaryTable::new(repeat_count))), // Array Descriptor (64-bit)
                    _ => Err(Error::StaticError("Ascii Table TFORM not recognized")),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let num_bits_per_row: u64 = self.tforms.iter().map(|tform| tform.num_bits_field()).sum();

        let num_bytes_per_row = num_bits_per_row >> 3;
        if num_bytes_per_row != self.naxis1 {
            return Err(Error::StaticError("BinTable NAXIS1 and TFORMS does not give the same amount of bytes the table should have per row."));
        }

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
            return Err(Error::StaticError(
                "Binary Table HDU must have a BITPIX = 8",
            ));
        }

        // NAXIS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis = parse_naxis_card(card_80_bytes_buf)?;
        if naxis != 2 {
            return Err(Error::StaticError("Binary Table HDU must have NAXIS = 2"));
        }

        // NAXIS1
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis1 =
            check_card_keyword(card_80_bytes_buf, NAXIS_KW[0])?.check_for_float()? as u64;
        // NAXIS2
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis2 =
            check_card_keyword(card_80_bytes_buf, NAXIS_KW[1])?.check_for_float()? as u64;

        // PCOUNT
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let pcount = parse_pcount_card(card_80_bytes_buf)?;

        // GCOUNT
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let gcount = parse_gcount_card(card_80_bytes_buf)?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let tfields =
            check_card_keyword(card_80_bytes_buf, b"TFIELDS ")?.check_for_float()? as usize;

        let tforms = vec![];

        Ok(BinTable {
            bitpix,
            naxis,
            naxis1,
            naxis2,
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
            return Err(Error::StaticError(
                "Binary Table HDU must have a BITPIX = 8",
            ));
        }

        // NAXIS
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let naxis = parse_naxis_card(card_80_bytes_buf)?;
        if naxis != 2 {
            return Err(Error::StaticError("Binary Table HDU must have NAXIS = 2"));
        }

        // NAXIS1
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let naxis1 =
            check_card_keyword(card_80_bytes_buf, NAXIS_KW[0])?.check_for_float()? as u64;

        // NAXIS2
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let naxis2 =
            check_card_keyword(card_80_bytes_buf, NAXIS_KW[1])?.check_for_float()? as u64;

        // PCOUNT
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let pcount = parse_pcount_card(card_80_bytes_buf)?;

        // GCOUNT
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let gcount = parse_gcount_card(card_80_bytes_buf)?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        consume_next_card_async(reader, card_80_bytes_buf, num_bytes_read).await?;
        let tfields =
            check_card_keyword(card_80_bytes_buf, b"TFIELDS ")?.check_for_float()? as usize;

        let tforms = vec![];

        Ok(BinTable {
            bitpix,
            naxis,
            naxis1,
            naxis2,
            tfields,
            tforms,
            pcount,
            gcount,
        })
    }
}

// More Xtension are defined in the original paper https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf
// See Appendix F

pub trait TFormType {
    const BITS_NUM: u64;
}

// Logical
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct L;
impl TFormType for L {
    const BITS_NUM: u64 = 8;
}
// Bit
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct X;
impl TFormType for X {
    const BITS_NUM: u64 = 1;
}
// Unsigned byte
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct B;
impl TFormType for B {
    const BITS_NUM: u64 = 8;
}
// 16-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct I;
impl TFormType for I {
    const BITS_NUM: u64 = 16;
}
// 32-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct J;
impl TFormType for J {
    const BITS_NUM: u64 = 32;
}
// 64-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct K;
impl TFormType for K {
    const BITS_NUM: u64 = 64;
}
// Character
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct A;
impl TFormType for A {
    const BITS_NUM: u64 = 8;
}
// Single-precision floating point
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct E;
impl TFormType for E {
    const BITS_NUM: u64 = 32;
}
// Double-precision floating point
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct D;
impl TFormType for D {
    const BITS_NUM: u64 = 64;
}
// Single-precision complex
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct C;
impl TFormType for C {
    const BITS_NUM: u64 = 64;
}
// Double-precision complex
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct M;
impl TFormType for M {
    const BITS_NUM: u64 = 128;
}
// Array Descriptor (32-bit)
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct P;
impl TFormType for P {
    const BITS_NUM: u64 = 64;
}
// Array Descriptor (64-bit)
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct Q;
impl TFormType for Q {
    const BITS_NUM: u64 = 128;
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct TFormBinaryTable<T>
where
    T: TFormType + Clone + Copy + Debug + Serialize + PartialEq,
{
    repeat_count: u64,
    phantom: std::marker::PhantomData<T>,
}

impl<T> TFormBinaryTable<T>
where
    T: TFormType + Clone + Copy + Debug + PartialEq + Serialize,
{
    pub fn new(repeat_count: u64) -> Self {
        Self {
            repeat_count,
            phantom: std::marker::PhantomData,
        }
    }

    pub fn get_repeat_count(&self) -> u64 {
        self.repeat_count
    }

    pub fn num_bits_field(&self) -> u64 {
        let ri = self.get_repeat_count();
        let bi = <T as TFormType>::BITS_NUM;

        ri * bi
    }
}

#[derive(PartialEq, Serialize, Clone, Copy, Debug)]
pub enum TFormBinaryTableType {
    L(TFormBinaryTable<L>), // Logical
    X(TFormBinaryTable<X>), // Bit
    B(TFormBinaryTable<B>), // Unsigned byte
    I(TFormBinaryTable<I>), // 16-bit integer
    J(TFormBinaryTable<J>), // 32-bit integer
    K(TFormBinaryTable<K>), // 64-bit integer
    A(TFormBinaryTable<A>), // Character
    E(TFormBinaryTable<E>), // Single-precision floating point
    D(TFormBinaryTable<D>), // Double-precision floating point
    C(TFormBinaryTable<C>), // Single-precision complex
    M(TFormBinaryTable<M>), // Double-precision complex
    P(TFormBinaryTable<P>), // Array Descriptor (32-bit)
    Q(TFormBinaryTable<Q>), // Array Descriptor (64-bit)
}

impl TFormBinaryTableType {
    fn num_bits_field(&self) -> u64 {
        match self {
            TFormBinaryTableType::L(tform) => tform.num_bits_field(), // Logical
            TFormBinaryTableType::X(tform) => tform.num_bits_field(), // Bit
            TFormBinaryTableType::B(tform) => tform.num_bits_field(), // Unsigned byte
            TFormBinaryTableType::I(tform) => tform.num_bits_field(), // 16-bit integer
            TFormBinaryTableType::J(tform) => tform.num_bits_field(), // 32-bit integer
            TFormBinaryTableType::K(tform) => tform.num_bits_field(), // 64-bit integer
            TFormBinaryTableType::A(tform) => tform.num_bits_field(), // Character
            TFormBinaryTableType::E(tform) => tform.num_bits_field(), // Single-precision floating point
            TFormBinaryTableType::D(tform) => tform.num_bits_field(), // Double-precision floating point
            TFormBinaryTableType::C(tform) => tform.num_bits_field(), // Single-precision complex
            TFormBinaryTableType::M(tform) => tform.num_bits_field(), // Double-precision complex
            TFormBinaryTableType::P(tform) => tform.num_bits_field(), // Array Descriptor (32-bit)
            TFormBinaryTableType::Q(tform) => tform.num_bits_field(), // Array Descriptor (64-bit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BinTable, TFormBinaryTable, TFormBinaryTableType};
    use crate::{
        fits::Fits,
        hdu::{extension::XtensionHDU, header::BitpixValue},
    };
    use std::{fs::File, io::BufReader};

    fn compare_bintable_ext(filename: &str, bin_table: BinTable) {
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
            XtensionHDU::BinTable(hdu) => {
                let xtension = hdu.get_header().get_xtension();
                assert_eq!(xtension.clone(), bin_table);
            }
            _ => panic!("Should contain a BinTable table HDU extension"),
        }
    }

    // These tests have been manually created thanks to this command on the fits files:
    // strings  samples/fits.gsfc.nasa.gov/HST_HRS.fits | fold -80 | grep "TBCOL" | tr -s ' ' | cut -d ' ' -f 3
    #[test]
    fn test_bintable_extension() {
        compare_bintable_ext(
            "samples/fits.gsfc.nasa.gov/IUE_LWP.fits",
            BinTable {
                bitpix: BitpixValue::U8,
                naxis: 2,
                naxis1: 11535,
                naxis2: 1,
                tfields: 9,
                tforms: vec![
                    TFormBinaryTableType::A(TFormBinaryTable::new(5)),
                    TFormBinaryTableType::I(TFormBinaryTable::new(1)),
                    TFormBinaryTableType::E(TFormBinaryTable::new(1)),
                    TFormBinaryTableType::E(TFormBinaryTable::new(1)),
                    TFormBinaryTableType::E(TFormBinaryTable::new(640)),
                    TFormBinaryTableType::E(TFormBinaryTable::new(640)),
                    TFormBinaryTableType::E(TFormBinaryTable::new(640)),
                    TFormBinaryTableType::I(TFormBinaryTable::new(640)),
                    TFormBinaryTableType::E(TFormBinaryTable::new(640)),
                ],
                // Should be 0
                pcount: 0,
                // Should be 1
                gcount: 1,
            },
        );
    }
}
