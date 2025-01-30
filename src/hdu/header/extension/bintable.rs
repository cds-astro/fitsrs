use std::collections::HashMap;
use std::fmt::Debug;

use async_trait::async_trait;
use serde::Serialize;
use crate::hdu::Value;
use crate::error::Error;
use crate::hdu::header::Bitpix;
use crate::hdu::header::check_for_bitpix;
use crate::hdu::header::check_for_gcount;
use crate::hdu::header::check_for_naxis;
use crate::hdu::header::check_for_naxisi;
use crate::hdu::header::check_for_pcount;
use crate::hdu::header::check_for_tfields;

use super::Xtension;

use log::warn;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct BinTable {
    bitpix: Bitpix,
    /// Number of axis, Should be 2,
    naxis: u64,
    /// A non-negative integer, giving the number of eight-bit bytes in each row of the
    /// table.
    pub(crate) naxis1: u64,
    /// A non-negative integer, giving the number of rows in the table
    pub(crate) naxis2: u64,
    /// A non-negative integer representing the number of fields in each row.
    /// The maximum permissible value is 999.
    pub(crate) tfields: usize,

    /// The value field of this keyword shall contain
    /// an integer providing the separation, in bytes, between the start
    /// of the main data table and the start of a supplemental data area
    /// called the heap. The default value, which is also the minimum
    /// allowed value, shall be the product of the values of NAXIS1 and
    /// NAXIS2. This keyword shall not be used if the value of PCOUNT
    /// is 0. The use sof this keyword is described in in Sect. 7.3.5.
    pub(crate) theap: usize,
    /// Contain a character string describing the format in which Field n is encoded.
    /// Only the formats in Table 15, interpreted as Fortran (ISO 2004)
    /// input formats and discussed in more detail in Sect. 7.2.5, are
    /// permitted for encoding
    pub(crate) tforms: Vec<TFormType>,

    /// TTYPEn keywords. The value field for this indexed keyword
    /// shall contain a character string giving the name of Field n. It
    /// is strongly recommended that every field of the table be assigned a unique, case-insensitive name with this keyword, and
    /// it is recommended that the character string be composed only
    /// of upper- and lower-case letters, digits, and the underscore (’ ’,
    /// decimal 95, hexadecimal 5F) character. Use of other characters is
    /// not recommended because it may be difficult to map the column
    /// names into variables in some languages (e.g., any hyphens, ’*’
    /// or ’+’ characters in the name may be confused with mathematical operators). String comparisons with the TTYPEn keyword
    /// values should not be case sensitive (e.g., ’TIME’ and ’Time’
    /// should be interpreted as the same name).
    pub(crate) ttypes: Vec<Option<String>>, 

    /// The value field shall contain the number of
    /// bytes that follow the table in the supplemental data area called
    /// the heap.
    pcount: u64,
    /// The value field shall contain the integer 1;
    /// the data blocks contain a single table.
    gcount: u64,

    /// ZIMAGE (required keyword) This keyword must have the logical value T. It indicates that the
    /// FITS binary table extension contains a compressed image and that logically this extension
    /// should be interpreted as an image and not as a table.
    pub(crate) z_image: Option<TileCompressedImage>
}

#[derive(Debug, PartialEq, Serialize, Clone)]
pub(crate) struct TileCompressedImage {
    /// ZCMPTYPE (required keyword) The value field of this keyword shall contain a character string
    /// giving the name of the algorithm that must be used to decompress the image. Currently, values of GZIP 1, GZIP 2, RICE 1, PLIO 1, and HCOMPRESS 1 are reserved, and the corresponding
    /// algorithms are described in a later section of this document. The value RICE ONE is also
    /// reserved as an alias for RICE 1.
    pub(crate) z_cmp_type: ZCmpType,

    /// ZBITPIX (required keyword) The value field of this keyword shall contain an integer that gives
    /// the value of the BITPIX keyword in the uncompressed FITS image.
    pub(crate) z_bitpix: Bitpix,

    /// ZNAXIS (required keyword) The value field of this keyword shall contain an integer that gives
    /// the value of the NAXIS keyword in the uncompressed FITS image.
    pub(crate) z_naxis: usize,

    /// ZNAXISn (required keywords) The value field of these keywords shall contain a positive integer
    /// that gives the value of the NAXISn keywords in the uncompressed FITS image.
    pub(crate) z_naxisn: Box<[usize]>,

    /// ZTILEn (optional keywords) The value of these indexed keywords (where n ranges from 1 to
    /// ZNAXIS) shall contain a positive integer representing the number of pixels along axis n of
    /// the compression tiles. Each tile of pixels is compressed separately and stored in a row of a
    /// variable-length vector column in the binary table. The size of each image dimension (given
    /// by ZNAXISn) is not required to be an integer multiple of ZTILEn, and if it is not, then the last
    /// tile along that dimension of the image will contain fewer image pixels than the other tiles.
    /// If the ZTILEn keywords are not present then the default ’row by row’ tiling will be assumed
    /// such that ZTILE1 = ZNAXIS1, and the value of all the other ZTILEn keywords equals 1.
    /// The compressed image tiles are stored in the binary table in the same order that the first pixel
    /// in each tile appears in the FITS image; the tile containing the first pixel in the image appears
    /// in the first row of the table, and the tile containing the last pixel in the image appears in the
    /// last row of the binary table.
    pub(crate) z_tilen: Box<[usize]>,

    /// ZQUANTIZ (optional keyword) This keyword records the name of the algorithm that was
    /// used to quantize floating-point image pixels into integer values which are then passed to
    /// the compression algorithm, as discussed further in section 4 of this document.
    pub(crate) z_quantiz: Option<ZQuantiz>,

    /// ZDITHER0 (optional keyword) The value field of this keyword shall contain an integer that
    /// gives the seed value for the random dithering pattern that was used when quantizing the
    /// floating-point pixel values. The value may range from 1 to 10000, inclusive. See section 4 for
    /// further discussion of this keyword.
    pub(crate) z_dither_0: Option<i64>
}

#[derive(Debug, PartialEq, Serialize, Clone)]
pub(crate) enum ZQuantiz {
    NoDither,
    SubtractiveDither1,
    SubtractiveDither2,
}

#[derive(Debug, PartialEq, Serialize, Clone, Copy)]
pub(crate) enum ZCmpType {
    Gzip1,
    Gzip2,
    Rice,
    PLI0_1,
    Hcompress1
}

#[async_trait(?Send)]
impl Xtension for BinTable {
    /// The table header consists of one or more 2880-byte header
    /// blocks with the last block indicated by the keyword END somewhere in the block. The main data table begins with the first data
    /// block following the last header block and is NAXIS1 × NAXIS2
    /// bytes in length. The zero-indexed byte offset to the start of
    /// the heap, measured from the start of the main data table, may
    /// be given by the THEAP keyword in the header. If this keyword is missing then the heap begins with the byte immediately
    /// following main data table (i.e., the default value of THEAP is
    /// NAXIS1 × NAXIS2). This default value is the minimum allowed
    /// value for the THEAP keyword, because any smaller value would
    /// imply that the heap and the main data table overlap. If the THEAP
    /// keyword has a value larger than this default value, then there is
    /// a gap between the end of the main data table and the start of
    /// the heap. The total length in bytes of the supplemental data area
    /// following the main data table (gap plus heap) is given by the
    /// PCOUNT keyword in the table header.
    fn get_num_bytes_data_block(&self) -> u64 {
        self.naxis1 * self.naxis2 + self.pcount
    }

    fn parse(
        values: &HashMap<String, Value>,
    ) -> Result<Self, Error> {
        // BITPIX
        let bitpix = check_for_bitpix(values)?;
        if bitpix != Bitpix::U8 {
            return Err(Error::StaticError(
                "Binary Table HDU must have a BITPIX = 8",
            ));
        }

        // NAXIS
        let naxis = check_for_naxis(values)? as u64;
        if naxis != 2 {
            return Err(Error::StaticError("Binary Table HDU must have NAXIS = 2"));
        }

        // NAXIS1
        let naxis1 = check_for_naxisi(values, 1)? as u64;
        // NAXIS2
        let naxis2 = check_for_naxisi(values, 2)? as u64;


        // PCOUNT
        let pcount = check_for_pcount(values)? as u64;

        // GCOUNT
        let gcount = check_for_gcount(values)? as u64;
        if gcount != 1 {
            return Err(Error::StaticError("Ascii Table HDU must have GCOUNT = 1"));
        }

        // FIELDS
        let tfields = check_for_tfields(values)?;

        // Tile compressed image parameters
        let z_cmp_type = if let Some(Value::String{value: ref z_cmp_type, ..}) = values.get("ZCMPTYPE") {
            match z_cmp_type.trim_ascii_end() {
                "GZIP_1" => Some(ZCmpType::Gzip1),
                "GZIP_2" => Some(ZCmpType::Gzip2),
                "RICE_1" | "RICE_ONE" => Some(ZCmpType::Rice),
                "PLI0_1" => Some(ZCmpType::PLI0_1),
                "HCOMPRESS_1" => Some(ZCmpType::Hcompress1),
                _ => {
                    warn!("ZCMPTYPE is not valid. The tile compressed image column will be discarded if any");
                    None
                }
            }
        } else {
            None
        };

        let z_bitpix = if let Some(Value::Integer{value: z_bitpix, ..}) = values.get("ZBITPIX") {
            match z_bitpix {
                8 => Some(Bitpix::U8),
                16 => Some(Bitpix::I16),
                32 => Some(Bitpix::I32),
                64 => Some(Bitpix::I64),
                -32 => Some(Bitpix::F32),
                -64 => Some(Bitpix::F64),
                _ => {
                    warn!("ZBITPIX is not valid. The tile compressed image column will be discarded if any");
                    None
                }
            }
        } else {
            None
        };

        // ZNAXIS (required keyword) The value field of this keyword shall contain an integer that gives
        // the value of the NAXIS keyword in the uncompressed FITS image.
        let z_naxis =  if let Some(Value::Integer{value, ..}) = values.get("ZNAXIS") {
            Some(*value)
        } else {
            None
        };

        // ZNAXISn (required keywords) The value field of these keywords shall contain a positive integer
        // that gives the value of the NAXISn keywords in the uncompressed FITS image.
        //
        // ZTILEn (optional keywords) The value of these indexed keywords (where n ranges from 1 to
        // ZNAXIS) shall contain a positive integer representing the number of pixels along axis n of
        // the compression tiles. Each tile of pixels is compressed separately and stored in a row of a
        // variable-length vector column in the binary table. The size of each image dimension (given
        // by ZNAXISn) is not required to be an integer multiple of ZTILEn, and if it is not, then the last
        // tile along that dimension of the image will contain fewer image pixels than the other tiles.
        // If the ZTILEn keywords are not present then the default ’row by row’ tiling will be assumed
        // such that ZTILE1 = ZNAXIS1, and the value of all the other ZTILEn keywords equals 1.
        // The compressed image tiles are stored in the binary table in the same order that the first pixel
        // in each tile appears in the FITS image; the tile containing the first pixel in the image appears
        // in the first row of the table, and the tile containing the last pixel in the image appears in the
        // last row of the binary table.
        let (z_naxisn, z_tilen) = if let Some(z_naxis) = z_naxis {
            let mut z_naxisn = Vec::with_capacity(z_naxis as usize);
            let mut z_tilen = Vec::with_capacity(z_naxis as usize);

            for i in 1..=(z_naxis as i64) {
                let naxisn = if let Some(Value::Integer{value, ..}) = values.get(&format!("ZNAXIS{i}")) {
                    Some(*value)
                } else {
                    None
                };

                if naxisn.is_none() {
                    warn!("ZNAXISN is mandatory. Tile compressed image discarded");
                    break;
                }

                let naxisn = naxisn.unwrap();
                // If not found, z_tilen equals z_naxisn
                let tilen = if let Some(Value::Integer{value, ..}) = values.get(&format!("ZNAXIS{i}")) {
                    *value
                } else {
                    naxisn
                };

                z_naxisn.push(naxisn as usize);
                z_tilen.push(tilen as usize)
            }

            if z_naxisn.len() != z_naxis as usize {
                (None, None)
            } else {
                (Some(z_naxisn.into_boxed_slice()), Some(z_tilen.into_boxed_slice()))
            }
        } else {
            (None, None)
        };

        // ZQUANTIZ (optional keyword) This keyword records the name of the algorithm that was
        // used to quantize floating-point image pixels into integer values which are then passed to
        // the compression algorithm, as discussed further in section 4 of this document.
        let z_quantiz = if let Some(Value::String { value: z_quantiz, .. }) = values.get("ZQUANTIZ") {
            match z_quantiz.trim_ascii_end() {
                "NO_DITHER" => Some(ZQuantiz::NoDither),
                "SUBTRACTIVE_DITHER_1" => Some(ZQuantiz::SubtractiveDither1),
                "SUBTRACTIVE_DITHER_2" => Some(ZQuantiz::SubtractiveDither2),
                _ => {
                    warn!("ZQUANTIZ value not recognized");
                    None
                }
            }
        } else {
            None
        };

        // ZDITHER0 (optional keyword) The value field of this keyword shall contain an integer that
        // gives the seed value for the random dithering pattern that was used when quantizing the
        // floating-point pixel values. The value may range from 1 to 10000, inclusive. See section 4 for
        // further discussion of this keyword.
        let z_dither_0 = if let Some(Value::Integer{value, ..}) = values.get(&format!("ZDITHER0")) {
            Some(*value)
        } else {
            None
        };

        // Fill the headers with these specific tile compressed image keywords
        let z_image = if let (Some(z_cmp_type), Some(z_bitpix), Some(z_naxis), Some(z_naxisn), Some(z_tilen)) = (z_cmp_type, z_bitpix, z_naxis, z_naxisn, z_tilen) {
            Some(TileCompressedImage { z_cmp_type, z_bitpix, z_naxis: z_naxis as usize, z_naxisn, z_tilen, z_quantiz, z_dither_0 })
        } else {
            None
        };

        // TFORMS
        let (tforms, ttypes): (Vec<_>, Vec<_>) = (0..tfields)
            .filter_map(|idx_field| {
                let idx_field = idx_field + 1;
                // discard the tform if it was not found and raise a warning
                let tform_kw = format!("TFORM{idx_field}");
                let tform = if let Some(Value::String{value, ..}) = values.get(&tform_kw) {
                    Some(value.to_owned())
                } else {
                    warn!("{} has not been found. It will be discarded", &tform_kw);

                    None
                }?;
                // try to find a ttype (optional keyword)
                let ttype = if let Some(Value::String{value, ..}) = values.get(&format!("TTYPE{idx_field}")) {
                    Some(value.to_owned())
                } else {
                    None
                };

                if ttype.is_none() {
                    warn!("Field {:?} does not have a TTYPE name.", tform_kw);
                }

                let count = tform
                    .chars()
                    .take_while(|c| c.is_digit(10))
                    .collect::<String>();

                let num_count_digits = count.len();
                let repeat_count = count.parse::<i64>().unwrap_or(1) as usize;
                // If the field type is not found, discard it as well
                let field_ty = tform.chars().nth(num_count_digits);
                if field_ty.is_none() {
                    warn!("Cannot extract the field type of {}", &tform_kw);
                }
                let field_ty = field_ty?;

                let compute_ty_array_desc = || {
                    // Get the type element of the stored array
                    let elem_ty = tform.chars().nth(num_count_digits + 1);

                    if elem_ty.is_none() {
                        warn!("Could not extract the type from the array descriptor field. Discard {}", &tform_kw);
                    }

                    let elem_ty = elem_ty?;

                    let (t_byte_size, mut ty) = match elem_ty {
                        'L' => (L::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::L)),
                        'X' => (X::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::X)),
                        'B' => (B::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::B)),
                        'I' => (I::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::I)),
                        'J' => (J::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::J)),
                        'K' => (K::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::K)),
                        'A' => (A::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::A)),
                        'E' => (E::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::E)),
                        'D' => (D::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::D)),
                        'C' => (C::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::C)),
                        'M' => (M::BYTES_SIZE, ArrayDescriptorTy::Default(VariableArrayTy::M)),
                        _ => {
                            warn!("Type not recognized. Discard {}", &tform_kw);
                            return None;
                        },
                    };

                    // Check whether it refers to a tile compressed image
                    ty = match (ttype.as_deref(), &z_image)  {
                        // GZIP1 byte
                        (Some("COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Gzip1, z_bitpix: Bitpix::U8, .. })) | (Some("GZIP_COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Gzip1, z_bitpix: Bitpix::U8, .. })) =>
                            ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::Gzip1U8),
                        // GZIP1 short
                        (Some("COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Gzip1, z_bitpix: Bitpix::I16, .. })) | (Some("GZIP_COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Gzip1, z_bitpix: Bitpix::I16, .. })) =>
                            ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::Gzip1I16),
                        // GZIP1 integer
                        (Some("COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Gzip1, z_bitpix: Bitpix::I32, .. })) | (Some("GZIP_COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Gzip1, z_bitpix: Bitpix::I32, .. })) =>
                            ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::Gzip1I32),
                        // RICE byte
                        (Some("COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Rice, z_bitpix: Bitpix::U8, .. })) =>
                            ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::RiceU8),
                        // RICE short
                        (Some("COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Rice, z_bitpix: Bitpix::I16, .. })) =>
                            ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::RiceI16),
                        // RICE integer
                        (Some("COMPRESSED_DATA"), Some(TileCompressedImage { z_cmp_type: ZCmpType::Rice, z_bitpix: Bitpix::I32, .. })) =>
                            ArrayDescriptorTy::TileCompressedImage(TileCompressedImageTy::RiceI32),
                        // consider the array as normal
                        _ => ty
                    };

                    Some((t_byte_size, ty))
                };

                let tformty = match field_ty as char {
                    // Logical
                    'L' => TFormType::L { repeat_count },
                    // Bit
                    'X' => TFormType::X { repeat_count },
                    // Unsigned Byte
                    'B' => TFormType::B { repeat_count },
                    // 16-bit integer
                    'I' => TFormType::I { repeat_count }, 
                    // 32-bit integer
                    'J' => TFormType::J { repeat_count }, 
                    // 64-bit integer
                    'K' => TFormType::K { repeat_count }, 
                    // Character
                    'A' => TFormType::A { repeat_count },
                    // Single-precision floating point
                    'E' => TFormType::E { repeat_count },
                    // Double-precision floating point
                    'D' => TFormType::D { repeat_count },
                    // Single-precision complex
                    'C' => TFormType::C { repeat_count },
                    // Double-precision complex
                    'M' => TFormType::M { repeat_count }, 
                    // Array Descriptor 32-bit
                    'P' => {
                        let (t_byte_size, ty) = compute_ty_array_desc()?;

                        TFormType::P {
                            t_byte_size: t_byte_size as u64,
                            e_max: 999,
                            ty,
                        }
                    },
                    // Array Descriptor 64-bit
                    'Q' => {
                        let (t_byte_size, ty) = compute_ty_array_desc()?;

                        TFormType::Q {
                            t_byte_size: t_byte_size as u64,
                            e_max: 999,
                            ty,
                        }
                    },
                    _ => {
                        warn!("Field type not recognized. Discard {}", tform_kw);
                        return None;
                    }
                };

                Some((tformty, ttype))
            })
            .unzip();

        // update the value of theap if found
        let theap = if let Some(Value::Integer{value, ..}) = values.get("THEAP") {
            *value as usize
        } else {
            (naxis1 as usize) * (naxis2 as usize)
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
            ttypes,
            pcount,
            gcount,
            theap,
            z_image
        })
    }
}

// More Xtension are defined in the original paper https://fits.gsfc.nasa.gov/standard40/fits_standard40aa-le.pdf
// See Appendix F

pub trait TForm {
    /// Number of bit associated to the TFORM
    const BITS_SIZE: usize;
    /// Number of bytes needed to store the tform
    const BYTES_SIZE: usize = (Self::BITS_SIZE as usize + 8 - 1) / 8;

    /// Rust type associated to the TFORM
    type Ty;
}
// Logical
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct L;
impl TForm for L {
    const BITS_SIZE: usize = 8;

    type Ty = u8;
}
// Bit
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct X;
impl TForm for X {
    const BITS_SIZE: usize = 1;

    type Ty = bool;
}
// Unsigned byte
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct B;
impl TForm for B {
    const BITS_SIZE: usize = 8;

    type Ty = u8;
}
// 16-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct I;
impl TForm for I {
    const BITS_SIZE: usize = 16;

    type Ty = i16;
}
// 32-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct J;
impl TForm for J {
    const BITS_SIZE: usize = 32;

    type Ty = i32;

}
// 64-bit integer
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct K;
impl TForm for K {
    const BITS_SIZE: usize = 64;

    type Ty = i64;
}
// Character
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct A;
impl TForm for A {
    const BITS_SIZE: usize = 8;

    type Ty = char;
}
// Single-precision floating point
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct E;
impl TForm for E {
    const BITS_SIZE: usize = 32;

    type Ty = f32;
}
// Double-precision floating point
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct D;
impl TForm for D {
    const BITS_SIZE: usize = 64;

    type Ty = f64;
}
// Single-precision complex
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub struct C;
impl TForm for C {
    const BITS_SIZE: usize = 64;

    type Ty = (f32, f32);
}
// Double-precision complex
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub(crate) struct M;
impl TForm for M {
    const BITS_SIZE: usize = 128;

    type Ty = (f64, f64);
}

// Array Descriptor (32-bit)
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub(crate) struct P;
impl TForm for P {
    const BITS_SIZE: usize = 64;

    type Ty = u64;
}

// Array Descriptor (64-bit)
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Default)]
pub(crate)  struct Q;
impl TForm for Q {
    const BITS_SIZE: usize = 128;

    type Ty = u128;
}

#[derive(PartialEq, Serialize, Clone, Copy, Debug)]
pub(crate) enum TFormType {
    /// Logical
    L {
        repeat_count: usize,
    },
    // Bit
    X {
        repeat_count: usize,
    },
    // Unsigned byte
    B {
        repeat_count: usize,
    },
    // 16-bit integer
    I {
        repeat_count: usize,
    },
    // 32-bit integer
    J {
        repeat_count: usize,
    },
    // 64-bit integer
    K {
        repeat_count: usize
    },
    // Character
    A {
        repeat_count: usize
    },
    // Single-precision floating point
    E {
        repeat_count: usize
    },
    // Double-precision floating point
    D {
        repeat_count: usize,
    },
    // Single-precision complex
    C {
        repeat_count: usize
    },
    // Double-precision complex
    M {
        repeat_count: usize
    },
    // Array Descriptor (32-bit) 
    P {
        /// number of bytes per element
        t_byte_size: u64,
        /// max number of elements of type t
        e_max: u64,
        /// the type
        ty: ArrayDescriptorTy,
    },
    // Array Descriptor (64-bit)
    Q {
        /// number of bytes per element
        t_byte_size: u64,
        /// max number of elements of type t
        e_max: u64,
        /// the type
        ty: ArrayDescriptorTy,
    }
}

#[derive(PartialEq, Serialize, Clone, Copy, Debug)]
pub(crate) enum TileCompressedImageTy {
    Gzip1U8,
    Gzip1I16,
    Gzip1I32,
    RiceU8,
    RiceI16,
    RiceI32,
}

#[derive(PartialEq, Serialize, Clone, Copy, Debug)]
pub(crate) enum VariableArrayTy {
    /// Logical
    L,
    // Bit
    X,
    // Unsigned byte
    B,
    // 16-bit integer
    I,
    // 32-bit integer
    J,
    // 64-bit integer
    K,
    // Character
    A,
    // Single-precision floating point
    E,
    // Double-precision floating point
    D,
    // Single-precision complex
    C,
    // Double-precision complex
    M,
}

#[derive(PartialEq, Serialize, Clone, Copy, Debug)]
pub(crate) enum ArrayDescriptorTy {
    TileCompressedImage(TileCompressedImageTy),
    Default(VariableArrayTy)
}

impl TFormType {
    pub(crate) fn num_bits_field(&self) -> usize {
        match self {
            TFormType::L { repeat_count } => repeat_count * L::BITS_SIZE, // Logical
            TFormType::X { repeat_count } => repeat_count * X::BITS_SIZE, // Bit
            TFormType::B { repeat_count } => repeat_count * B::BITS_SIZE, // Unsigned byte
            TFormType::I { repeat_count } => repeat_count * I::BITS_SIZE, // 16-bit integer
            TFormType::J { repeat_count } => repeat_count * J::BITS_SIZE, // 32-bit integer
            TFormType::K { repeat_count } => repeat_count * K::BITS_SIZE, // 64-bit integer
            TFormType::A { repeat_count } => repeat_count * A::BITS_SIZE, // Character
            TFormType::E { repeat_count } => repeat_count * E::BITS_SIZE, // Single-precision floating point
            TFormType::D { repeat_count } => repeat_count * D::BITS_SIZE, // Double-precision floating point
            TFormType::C { repeat_count } => repeat_count * C::BITS_SIZE, // Single-precision complex
            TFormType::M { repeat_count } => repeat_count * M::BITS_SIZE, // Double-precision complex
            TFormType::P { .. } => P::BITS_SIZE,                                  // Array Descriptor (32-bit)
            TFormType::Q { .. } => Q::BITS_SIZE,                                  // Array Descriptor (64-bit)
        }
    }

    pub(crate) fn num_bytes_field(&self) -> usize {
        (self.num_bits_field() as usize + 8 - 1) / 8
    }
}

#[cfg(test)]
mod tests {
    use super::{BinTable, TFormType};
    use crate::{
        hdu::{header::Bitpix, HDU}, FITSFile,
    };

    fn compare_bintable_ext(filename: &str, bin_table: BinTable) {
        let f = FITSFile::open(filename).unwrap();

        //let reader = BufReader::new(f);
        //let hdu_list = Fits::from_reader(reader);

        // Get the first HDU extension,
        // this should be the table for these fits examples
        let hdu = f
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
                bitpix: Bitpix::U8,
                naxis: 2,
                naxis1: 11535,
                naxis2: 1,
                tfields: 9,
                tforms: vec![
                    TFormType::A { repeat_count: 5 },
                    TFormType::I { repeat_count: 1 },
                    TFormType::E { repeat_count: 1 },
                    TFormType::E { repeat_count: 1 },
                    TFormType::E { repeat_count: 640 },
                    TFormType::E { repeat_count: 640 },
                    TFormType::E { repeat_count: 640 },
                    TFormType::I { repeat_count: 640 },
                    TFormType::E { repeat_count: 640 },
                ],
                ttypes: vec![Some("APERTURE".to_owned()), Some("NPOINTS".to_owned()), Some("WAVELENGTH".to_owned()), Some("DELTAW".to_owned()), Some("NET".to_owned()), Some("BACKGROUND".to_owned()), Some("SIGMA".to_owned()), Some("QUALITY".to_owned()), Some("FLUX".to_owned())],
                theap: 11535,
                // Should be 0
                pcount: 0,
                // Should be 1
                gcount: 1,
                z_image: None,
            },
        );
    }
}
