use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use serde::Serialize;

use crate::error::Error;
use crate::hdu::header::BitpixValue;
use crate::hdu::header::check_for_bitpix;
use crate::hdu::header::check_for_gcount;
use crate::hdu::header::check_for_naxis;
use crate::hdu::header::check_for_naxisi;

use crate::card::Value;
use crate::hdu::header::check_for_pcount;
use crate::hdu::header::check_for_tfields;

use super::Xtension;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct BinTable {
    bitpix: BitpixValue,
    // Number of axis, Should be 2,
    naxis: usize,
    // A non-negative integer, giving the number of eight-bit bytes in each row of the
    // table.
    pub(crate) naxis1: u64,
    // A non-negative integer, giving the number of rows in the table
    naxis2: u64,
    // A non-negative integer representing the number of fields in each row.
    // The maximum permissible value is 999.
    tfields: usize,

    // The value field of this keyword shall contain
    // an integer providing the separation, in bytes, between the start
    // of the main data table and the start of a supplemental data area
    // called the heap. The default value, which is also the minimum
    // allowed value, shall be the product of the values of NAXIS1 and
    // NAXIS2. This keyword shall not be used if the value of PCOUNT
    // is 0. The use of this keyword is described in in Sect. 7.3.5.
    pub(crate) theap: usize,
    // Contain a character string describing the format in which Field n is encoded.
    // Only the formats in Table 15, interpreted as Fortran (ISO 2004)
    // input formats and discussed in more detail in Sect. 7.2.5, are
    // permitted for encoding
    pub(crate) tforms: Vec<TFormBinaryTableType>,

    // The value field shall contain the number of
    // bytes that follow the table in the supplemental data area called
    // the heap.
    pcount: usize,
    // The value field shall contain the integer 1;
    // the data blocks contain a single table.
    gcount: usize,
}

#[async_trait(?Send)]
impl Xtension for BinTable {
    fn get_num_bytes_data_block(&self) -> u64 {
        self.naxis1 * self.naxis2
    }

    fn parse(
        values: &HashMap<String, Value>,
    ) -> Result<Self, Error> {
        // BITPIX
        let bitpix = check_for_bitpix(values)?;
        if bitpix != BitpixValue::U8 {
            return Err(Error::StaticError(
                "Binary Table HDU must have a BITPIX = 8",
            ));
        }

        // NAXIS
        let naxis = check_for_naxis(values)?;
        if naxis != 2 {
            return Err(Error::StaticError("Binary Table HDU must have NAXIS = 2"));
        }

        // NAXIS1
        let naxis1 = check_for_naxisi(values, 1)? as u64;
        // NAXIS2
        let naxis2 = check_for_naxisi(values, 2)? as u64;


        // PCOUNT
        let pcount = check_for_pcount(values)?;

        // GCOUNT
        let gcount = check_for_gcount(values)?;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        let tfields = check_for_tfields(values)?;
        // TFORMS
        let tforms = (0..tfields)
            .map(|idx_field| {
                let idx_field = idx_field + 1;
                let kw = format!("TFORM{idx_field:?}");

                let card_value = values
                    .get(&kw)
                    .ok_or(Error::StaticError("TFIELDS idx does not map any TFORM!"))?
                    .clone()
                    .check_for_string()?;

                let count = card_value
                    .chars()
                    .take_while(|c| c.is_digit(10))
                    .collect::<String>();

                let num_count_digits = count.len();
                let count = count.parse::<i64>().unwrap_or(1) as usize;
                let field_ty = card_value.chars().nth(num_count_digits).unwrap();

                match field_ty as char {
                    'L' => Ok(TFormBinaryTableType::L(TFormBinaryTable::new(count))), // Logical
                    'X' => Ok(TFormBinaryTableType::X(TFormBinaryTable::new(count))), // Bit
                    'B' => Ok(TFormBinaryTableType::B(TFormBinaryTable::new(count))), // Unsigned Byte
                    'I' => Ok(TFormBinaryTableType::I(TFormBinaryTable::new(count))), // 16-bit integer
                    'J' => Ok(TFormBinaryTableType::J(TFormBinaryTable::new(count))), // 32-bit integer
                    'K' => Ok(TFormBinaryTableType::K(TFormBinaryTable::new(count))), // 64-bit integer
                    'A' => Ok(TFormBinaryTableType::A(TFormBinaryTable::new(count))), // Character
                    'E' => Ok(TFormBinaryTableType::E(TFormBinaryTable::new(count))), // Single-precision floating point
                    'D' => Ok(TFormBinaryTableType::D(TFormBinaryTable::new(count))), // Double-precision floating point
                    'C' => Ok(TFormBinaryTableType::C(TFormBinaryTable::new(count))), // Single-precision complex
                    'M' => Ok(TFormBinaryTableType::M(TFormBinaryTable::new(count))), // Double-precision complex
                    'P' => {
                        // Get the type element of the stored array
                        let elem_ty = card_value.chars().nth(num_count_digits + 1).ok_or(
                            Error::StaticError(
                                "No element type found for variable-length array field",
                            ),
                        )?;
                        let p = TFormBinaryTable::new(count).set_additional_data(P {
                            t_byte_size: match elem_ty {
                                'B' => Ok(B::BYTES_SIZE), // byte
                                'I' => Ok(I::BYTES_SIZE), // short
                                'J' => Ok(J::BYTES_SIZE), // integer
                                'K' => Ok(K::BYTES_SIZE), // long
                                'E' => Ok(E::BYTES_SIZE), // float
                                'D' => Ok(D::BYTES_SIZE), // double
                                _ => Err(Error::StaticError(
                                    "Type not supported for elements in an array",
                                )),
                            }?,
                            e_max: 999,
                            ty: elem_ty,
                        });
                        Ok(TFormBinaryTableType::P(p))
                        // Array Descriptor (32-bit)
                    }
                    'Q' => {
                        // Get the type element of the stored array
                        let elem_ty = card_value.chars().nth(num_count_digits + 1).ok_or(
                            Error::StaticError(
                                "No element type found for variable-length array field",
                            ),
                        )?;
                        let q = TFormBinaryTable::new(count).set_additional_data(Q {
                            t_byte_size: match elem_ty {
                                'B' => Ok(B::BYTES_SIZE), // byte
                                'I' => Ok(I::BYTES_SIZE), // short
                                'J' => Ok(J::BYTES_SIZE), // integer
                                'K' => Ok(K::BYTES_SIZE), // long
                                'E' => Ok(E::BYTES_SIZE), // float
                                'D' => Ok(D::BYTES_SIZE), // double
                                _ => Err(Error::StaticError(
                                    "Type not supported for elements in an array",
                                )),
                            }?,
                            e_max: 999,
                        });
                        Ok(TFormBinaryTableType::Q(q))
                    } // Array Descriptor (64-bit)
                    _ => Err(Error::StaticError("Ascii Table TFORM not recognized")),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        // default value
        let theap = if let Some(Value::Integer { value, .. }) = values.get("THEAP") {
            *value as usize
        } else {
            // The default value means the HEAP begins right after the binary table ends
            (naxis1 * naxis2) as usize
        };

        let num_bits_per_row = tforms.iter().map(|tform| tform.num_bits_field() as u64).sum::<u64>();

        let num_bytes_per_row = num_bits_per_row >> 3;
        if num_bytes_per_row != naxis1 {
            return Err(Error::StaticError("BinTable NAXIS1 and TFORMS does not give the same amount of bytes the table should have per row."));
        }
      
        Ok(BinTable {
            bitpix,
            naxis,
            naxis1,
            naxis2,
            tfields,
            tforms,
            pcount,
            gcount,
            theap,
        })
    }
}

// More Xtension are defined in the original paper https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf
// See Appendix F

pub trait TFormType {
    const BITS_SIZE: usize;
    const BYTES_SIZE: usize = (Self::BITS_SIZE as usize + 8 - 1) / 8;
}

impl<T> TFormType for TFormBinaryTable<T>
where
    T: TFormType + Clone + Copy + Debug + Serialize + PartialEq + Default,
{
    const BITS_SIZE: usize = T::BITS_SIZE;
}

// Logical
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct L;
impl TFormType for L {
    const BITS_SIZE: usize = 8;
}
// Bit
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct X;
impl TFormType for X {
    const BITS_SIZE: usize = 1;
}
// Unsigned byte
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct B;
impl TFormType for B {
    const BITS_SIZE: usize = 8;
}
// 16-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct I;
impl TFormType for I {
    const BITS_SIZE: usize = 16;
}
// 32-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct J;
impl TFormType for J {
    const BITS_SIZE: usize = 32;
}
// 64-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct K;
impl TFormType for K {
    const BITS_SIZE: usize = 64;
}
// Character
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct A;
impl TFormType for A {
    const BITS_SIZE: usize = 8;
}
// Single-precision floating point
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct E;
impl TFormType for E {
    const BITS_SIZE: usize = 32;
}
// Double-precision floating point
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct D;
impl TFormType for D {
    const BITS_SIZE: usize = 64;
}
// Single-precision complex
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct C;
impl TFormType for C {
    const BITS_SIZE: usize = 64;
}
// Double-precision complex
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct M;
impl TFormType for M {
    const BITS_SIZE: usize = 128;
}
// Array Descriptor (32-bit)
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct P {
    // elem type
    pub(crate) t_byte_size: usize,
    // max number of elements of type t
    pub(crate) e_max: usize,

    pub(crate) ty: char,
}
impl TFormType for P {
    const BITS_SIZE: usize = 64;
}
// Array Descriptor (64-bit)
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct Q {
    // elem type
    t_byte_size: usize,
    // max number of elements of type t
    e_max: usize,
}
impl TFormType for Q {
    const BITS_SIZE: usize = 128;
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct TFormBinaryTable<T>
where
    T: TFormType + Clone + Copy + Debug + Serialize + PartialEq + Default,
{
    repeat_count: usize,
    // additional metadata for that form
    pub(crate) config: T,
}

impl<T> TFormBinaryTable<T>
where
    T: TFormType + Clone + Copy + Debug + PartialEq + Serialize + Default,
{
    pub fn new(repeat_count: usize) -> Self {
        Self {
            repeat_count,
            config: Default::default(),
        }
    }

    pub fn set_additional_data(mut self, config: T) -> Self {
        self.config = config;

        self
    }

    pub fn get_repeat_count(&self) -> usize {
        self.repeat_count
    }

    pub fn num_bits_field(&self) -> usize {
        let ri = self.get_repeat_count();
        let bi = <T as TFormType>::BITS_SIZE;

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
    pub(crate) fn num_bits_field(&self) -> usize {
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

    pub(crate) fn num_bytes_field(&self) -> usize {
        (self.num_bits_field() as usize + 8 - 1) / 8
    }
}

#[cfg(test)]
mod tests {
    use super::{BinTable, TFormBinaryTable, TFormBinaryTableType};
    use crate::{
        fits::Fits,
        hdu::{header::BitpixValue, HDU},
    };
    use std::{fs::File, io::BufReader};

    fn compare_bintable_ext(filename: &str, bin_table: BinTable) {
        let f = File::open(filename).unwrap();

        let reader = BufReader::new(f);
        let hdu_list = Fits::from_reader(reader);

        // Get the first HDU extension,
        // this should be the table for these fits examples
        let hdu = hdu_list
            // skip the primary hdu
            .skip(1)
            .next()
            .expect("Should contain an extension HDU")
            .unwrap();

        match hdu {
            HDU::XBinaryTable(hdu) => {
                let xtension = hdu.get_header().get_xtension();
                assert_eq!(xtension.clone(), bin_table);
            }
            _ => panic!("Should contain a BinTable table HDU extension"),
        }
    }

    // These tests have been manually created thanks to this command on the fits files:
    // strings  samples/fits.gsfc.nasa.gov/HST_HRS.fits | fold -80 | grep "TBCOL" | tr -s ' ' | cut -d ' ' -f 3
    #[test]
    fn test_fits_bintable_extension() {
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
                theap: 11535
            },
        );
    }
}
