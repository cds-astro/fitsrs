use byteorder::BigEndian;

pub mod buf;
pub mod cursor;
pub mod rice;

use std::borrow::Cow;

use std::io::{Read, Cursor};
use crate::byteorder::ReadBytesExt;

use crate::hdu::header::extension::bintable::BinTable;


/// A data structure refering to a column in a table
#[derive(Debug)]
pub enum ColumnId {
    /// The user can give a column index
    Index(usize),
    /// Or a name to refer a specific TTYPE keyword
    Name(String),
}

#[derive(Debug)]
pub enum DataValue {
    // 'L' => Logical
    Logical {
        /// The value read
        value: bool,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'X' => Bit
    Bit {
        /// The current byte where the bit lies
        byte: u8,
        /// The bit index in the byte
        bit_idx: u8,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'B' => Unsigned Byte
    UnsignedByte {
         /// The value read
         value: u8,
         /// Name of the column
         column: ColumnId,
         /// Its position in the column (i.e. when repeat count > 1)
         idx: usize,
    },
    // 'I' => 16-bit integer
    Short {
        /// The value read
        value: i16,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'J' => 32-bit integer
    Integer {
        /// The value read
        value: i32,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'K' => 64-bit integer
    Long {
        /// The value read
        value: i64,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'A' => Character
    Character {
        /// The value read
        value: char,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'E' => Single-precision floating point
    Float {
        /// The value read
        value: f32,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'D' => Double-precision floating point
    Double {
        /// The value read
        value: f64,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'C' => Single-precision complex
    ComplexFloat {
        /// The real part of the complex number
        real: f32,
        /// Its imaginary part
        imag: f32,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    // 'M' => Double-precision complex
    ComplexDouble {
        /// The real part of the complex number
        real: f64,
        /// Its imaginary part
        imag: f64,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    }
}
/*
impl DataValue {
    fn parse_variable_array(reader: R, elem_ty: char, limit: u64, _ctx: &BinTable) -> Self {
        #[cfg(feature="tci")]
        let field = {
            use crate::hdu::header::Bitpix;
            use crate::hdu::header::extension::bintable::ZCmpType;

            // TILE compressed convention case. More details here: https://fits.gsfc.nasa.gov/registry/tilecompression.html
            if let Some(z_image) = &_ctx.z_image {
                match (z_image.z_cmp_type, z_image.z_bitpix) {
                    // It can only store integer typed values i.e. bytes, short or integers
                    (ZCmpType::Gzip1, Bitpix::U8) => VariableArray::U8(EitherIt::second(BigEndianGzIt::new(reader, limit))),
                    (ZCmpType::Gzip1, Bitpix::I16) => VariableArray::I16(EitherIt::second(BigEndianGzIt::new(reader, limit))),
                    (ZCmpType::Gzip1, Bitpix::I32) => VariableArray::I32(EitherIt::second(BigEndianGzIt::new(reader, limit))),
                    // Return the slice of bytes
                    _ => VariableArray::U8(EitherIt::first(BigEndianIt::new(reader, limit)))
                }
            } else {
                // once we have the bytes, convert it accordingly to the type given by the array descriptor
                match elem_ty {
                    'I' => VariableArray::I16(EitherIt::first(BigEndianIt::new(reader, limit))),
                    'J' => VariableArray::I32(EitherIt::first(BigEndianIt::new(reader, limit))),
                    'K' => VariableArray::I64(BigEndianIt::new(reader, limit)),
                    'E' => VariableArray::F32(BigEndianIt::new(reader, limit)),
                    'D' => VariableArray::F64(BigEndianIt::new(reader, limit)),
                    _ => VariableArray::U8(EitherIt::first(BigEndianIt::new(reader, limit))),
                }
            }
        };
        #[cfg(not(feature="tci"))]
        let field = {
            // once we have the bytes, convert it accordingly
            match elem_ty {
                'I' => VariableArray::I16(EitherIt::first(BigEndianIt::new(reader, limit))),
                'J' => VariableArray::I32(EitherIt::first(BigEndianIt::new(reader, limit))),
                'K' => VariableArray::I64(BigEndianIt::new(reader, limit)),
                'E' => VariableArray::F32(BigEndianIt::new(reader, limit)),
                'D' => VariableArray::F64(BigEndianIt::new(reader, limit)),
                _ => VariableArray::U8(EitherIt::first(BigEndianIt::new(reader, limit))),
            }
        };

        FieldTy::VariableArray(field)
    }
}*/

//pub type Row<'a, R> = Vec<FieldTy<'a, R>>;

use crate::hdu::data::iter::{CastIt, EitherIt};
use flate2::read::GzDecoder;
/// An iterator generating T typed values by reading raw bytes in the big endian scheme.
//#[derive(Debug)]
//pub struct BigEndianByteIt<'a, T>(BigEndianIt<BytesReader<'a>, T>);
/// An iterator generating T typed values by reading gzipped bytes and converting them in the BE scheme.
#[derive(Debug)]
struct BigEndianGzIt<R, T>(CastIt<BigEndianIt<GzDecoder<R>, i32>, T>)
where
    R: Read;

impl<R, T> BigEndianGzIt<R, T>
where
    R: Read + Debug
{
    fn new(reader: R, limit: u64) -> Self {
        Self(CastIt::new(BigEndianIt::new(GzDecoder::new(reader), limit)))
    }
}

use crate::hdu::data::iter::Value;

impl<R, T> Iterator for BigEndianGzIt<R, T>
where
    R: Read,
    T: Value,
    CastIt<BigEndianIt<GzDecoder<R>, i32>, T>: Iterator
{
    type Item = <CastIt<BigEndianIt<GzDecoder<R>, i32>, T> as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}


use super::iter::BigEndianIt;
use std::fmt::Debug;
//#[cfg(feature="tci")]
/// This contains variable array content
#[derive(Debug)]
pub enum VariableArray<R>
where
    R: Read + Debug
{
    /// Iterator that will return bytes and decode the data for tile compressed images
    U8(EitherIt<
        BigEndianIt<R, u8>,
        BigEndianGzIt<R, u8>,
        u8
    >),
    /// Iterator that will return shorts and decode the data for tile compressed images
    I16(EitherIt<
        BigEndianIt<R, i16>,
        BigEndianGzIt<R, i16>,
        i16
    >),
    /// Iterator that will return integers and decode the data for tile compressed images
    I32(EitherIt<
        BigEndianIt<R, i32>,
        BigEndianGzIt<R, i32>,
        i32
    >),
    I64(BigEndianIt<R, i64>),
    F32(BigEndianIt<R, f32>),
    F64(BigEndianIt<R, f64>),
}
/*
#[cfg(not(feature="tci"))]
pub enum VariableArray<'a> {
    /// Return the bytes directly
    U8(Bytes<'a>),
    /// Iterator that will return shorts and decode the data for tile compressed images
    I16(EitherIt<
        BigEndianByteIt<'a, i16>,
        BigEndianGzByteIt<'a, i16>,
        i16
    >),
    /// Iterator that will return integers and decode the data for tile compressed images
    I32(EitherIt<
        BigEndianByteIt<'a, i32>,
        BigEndianGzByteIt<'a, i32>,
        i32
    >),
    I64(BigEndianByteIt<'a, i64>),
    F32(BigEndianByteIt<'a, f32>),
    F64(BigEndianByteIt<'a, f64>),
}*/
