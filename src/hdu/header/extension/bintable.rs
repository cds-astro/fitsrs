use std::io::Read;
use std::fmt::Debug;

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

#[derive(Debug, PartialEq)]
#[derive(Serialize)]
#[derive(Clone)]
pub struct BinTable {
    bitpix: BitpixValue,
    // Number of axis, Should be 2,
    naxis: usize,
    // A non-negative integer, giving the number of eight-bit bytes in each row of the
    // table.
    naxis1: usize,
    // A non-negative integer, giving the number of rows in the table
    naxis2: usize,
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

impl Xtension for BinTable {
    fn get_num_bytes_data_block(&self) -> usize {
        self.naxis1 * self.naxis2
    }

    fn parse<R: Read>(reader: &mut R, num_bytes_read: &mut usize, card_80_bytes_buf: &mut [u8; 80]) -> Result<Self, Error> {
        // BITPIX
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let bitpix = parse_bitpix_card(&card_80_bytes_buf)?;
        if bitpix != BitpixValue::U8 {
            return Err(Error::StaticError("Binary Table HDU must have a BITPIX = 8"));
        }

        // NAXIS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis = parse_naxis_card(&card_80_bytes_buf)?;

        if naxis != 2 {
            return Err(Error::StaticError("Binary Table HDU must have NAXIS = 2"));
        }
        // NAXIS1
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis1 = check_card_keyword(&card_80_bytes_buf, NAXIS_KW[0])?
            .check_for_float()? as usize;
        // NAXIS2
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let naxis2 = check_card_keyword(&card_80_bytes_buf, NAXIS_KW[1])?
            .check_for_float()? as usize;

        // GCOUNT
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let gcount = parse_gcount_card(&card_80_bytes_buf)?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // PCOUNT
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let pcount = parse_pcount_card(&card_80_bytes_buf)?;

        // FIELDS
        consume_next_card(reader, card_80_bytes_buf, num_bytes_read)?;
        let tfields = check_card_keyword(&card_80_bytes_buf, b"TFIELDS ")?
            .check_for_float()? as usize;

        // TFORMS
        /*let tforms: Vec<_> = (0..tfields)
            .map(|idx_field| {
                let idx_field = idx_field + 1;
                let kw = format!("TFORM{:?}       ", idx_field)
                    .as_str()
                    .as_bytes();

                // 1. Init the fixed keyword slice
                let mut owned_kw: [u8; 8] = [0; 8];
                // 2. Copy from slice
                owned_kw.copy_from_slice(&kw[..8]);

                check_card_keyword(&card_80_bytes_buf, &owned_kw)?
                    .check_for_string()
                    .map(|tform| {
                        match tform[0] {
                            'A' => {
                                let w = tforms[1..].parse::<i32>()
                                    .map_err(Error::StaticError("expected w after the "))?;
                                Ok(TFormAsciiTable::A { w })
                            },
                            'I' => {
                                let w = tforms[1..].parse::<i32>()
                                    .map_err(Error::StaticError("expected w after the "))?;
                                Ok(TFormAsciiTable::I { w })
                            },
                            'F' => {
                                let wd = tforms[1..]
                                    .to_string();
                                Ok(TFormAsciiTable::F { wd })
                            },
                            'E' => {
                                let wd = tforms[1..]
                                    .to_string();
                                Ok(TFormAsciiTable::E { wd })
                            },
                            'D' => {
                                let wd = tforms[1..]
                                    .to_string();
                                Ok(TFormAsciiTable::D { wd })
                            },
                            _ => Err(StaticError("Ascii Table TFORM not recognized"))
                        }
                    })
            })
            .collect()?;
        */
        let tforms = vec![];
        /*
        let num_bits_per_row = tforms.iter()
            .map(|tform| {
                tform.num_bits_field()
            })
            .sum();

        let num_bytes_per_row = num_bits_per_row >> 3;
        if num_bytes_per_row != self.naxis1 {
            return Err(Error::StaticError("BinTable NAXIS1 and TFORMS does not give the same amount of bytes the table should have per row."));
        }
        */

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
    const BITS_NUM: usize;
}

// Logical
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct L;
impl TFormType for L {
    const BITS_NUM: usize = 8;
}
// Bit
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct X;
impl TFormType for X {
    const BITS_NUM: usize = 1;
}
// Unsigned byte
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct B;
impl TFormType for B {
    const BITS_NUM: usize = 8;
}
// 16-bit integer
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct I;
impl TFormType for I {
    const BITS_NUM: usize = 16;
}
// 32-bit integer
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct J;
impl TFormType for J {
    const BITS_NUM: usize = 32;
}
// 64-bit integer
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct K;
impl TFormType for K {
    const BITS_NUM: usize = 64;
}
// Character
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct A;
impl TFormType for A {
    const BITS_NUM: usize = 8;
}
// Single-precision floating point
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct E;
impl TFormType for E {
    const BITS_NUM: usize = 32;
}
// Double-precision floating point
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct D;
impl TFormType for D {
    const BITS_NUM: usize = 64;
}
// Single-precision complex
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct C;
impl TFormType for C {
    const BITS_NUM: usize = 64;
}
// Double-precision complex
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct M;
impl TFormType for M {
    const BITS_NUM: usize = 128;
}
// Array Descriptor (32-bit)
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct P;
impl TFormType for P {
    const BITS_NUM: usize = 64;
}
// Array Descriptor (64-bit)
#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(Serialize)]
#[derive(PartialEq)]
pub struct Q;
impl TFormType for Q {
    const BITS_NUM: usize = 128;
}

#[derive(Clone, Copy)]
#[derive(Debug)]
#[derive(PartialEq)]
#[derive(Serialize)]
pub struct TFormBinaryTable<T>
where
    T: TFormType + Clone + Copy + Debug + Serialize + PartialEq
{
    repeat_count: usize,
    tform_type: T,
}

impl<T> TFormBinaryTable<T>
where
    T: TFormType + Clone + Copy + Debug + PartialEq + Serialize
{
    fn get_repeat_count(&self) -> usize {
        self.repeat_count
    }

    fn num_bits_field(&self) -> usize {
        let ri = self.get_repeat_count();
        let bi = <T as TFormType>::BITS_NUM;

        ri * bi
    }
}

#[derive(PartialEq)]
#[derive(Serialize)]
#[derive(Clone, Copy)]
#[derive(Debug)]
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
    fn num_bits_field(&self) -> usize {
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
