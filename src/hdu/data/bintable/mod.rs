#![allow(clippy::upper_case_acronyms)]

pub mod data;
mod deser;
pub mod row;
pub mod tile_compressed;

pub use data::TableData;
pub use row::TableRowData;

use crate::error::Error;
use serde::de::value::SeqDeserializer;
use serde::de::IntoDeserializer;
use serde::{forward_to_deserialize_any, Deserializer};

/// A data structure refering to a column in a table
#[derive(Clone)]
pub enum ColumnId {
    /// The user can give a column index
    Index(usize),
    /// Or a name to refer a specific TTYPE keyword
    Name(&'static str),
}

#[derive(Clone)]
pub enum DataValue {
    /// 'L' => Logical
    Logical {
        /// The value read
        value: bool,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'X' => Bit
    BitArray {
        /// The current byte where the bit lies
        byte: u8,
        /// The idx of the byte of the array
        idx: usize,
        /// Name of the column
        column: ColumnId,
    },
    /// 'B' => Unsigned Byte
    UnsignedByte {
        /// The value read
        value: u8,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'I' => 16-bit integer
    Short {
        /// The value read
        value: i16,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'J' => 32-bit integer
    Integer {
        /// The value read
        value: i32,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'K' => 64-bit integer
    Long {
        /// The value read
        value: i64,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'A' => Character
    Character {
        /// The value read
        value: char,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'E' => Single-precision floating point
    Float {
        /// The value read
        value: f32,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'D' => Double-precision floating point
    Double {
        /// The value read
        value: f64,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'C' => Single-precision complex
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
    /// 'M' => Double-precision complex
    ComplexDouble {
        /// The real part of the complex number
        real: f64,
        /// Its imaginary part
        imag: f64,
        /// Name of the column
        column: ColumnId,
        /// Its position in the column (i.e. when repeat count > 1)
        idx: usize,
    },
    /// 'P' => Variable-length array descriptor (32-bits case)
    VariableLengthArray32 {
        /// The number of elements in the array
        num_elems: u32,
        /// The offset byte position from the start of the heap
        offset_byte: u32,
    },
    /// 'Q' => Variable-length array descriptor (64-bits case)
    VariableLengthArray64 {
        /// The number of elements in the array
        num_elems: u64,
        /// The offset byte position from the start of the heap
        offset_byte: u64,
    },
    /// Some TFORM encodes NULL values, such as L, A (Null strings), B, I, J, K
    Null,
}

pub fn seq_deserializer<T, const N: usize>(
    values: [T; N],
) -> SeqDeserializer<std::array::IntoIter<T, N>, Error>
where
    T: IntoDeserializer<'static, Error>,
{
    SeqDeserializer::new(IntoIterator::into_iter(values))
}

impl<'de> Deserializer<'de> for DataValue {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            DataValue::Logical { value, .. } => visitor.visit_bool(value),
            DataValue::BitArray { byte, .. } => visitor.visit_u8(byte),
            DataValue::UnsignedByte { value, .. } => visitor.visit_u8(value),
            DataValue::Short { value, .. } => visitor.visit_i16(value),
            DataValue::Integer { value, .. } => visitor.visit_i32(value),
            DataValue::Long { value, .. } => visitor.visit_i64(value),
            DataValue::Character { value, .. } => visitor.visit_char(value),
            DataValue::Float { value, .. } => visitor.visit_f32(value),
            DataValue::Double { value, .. } => visitor.visit_f64(value),
            DataValue::ComplexFloat { real, imag, .. } => {
                visitor.visit_newtype_struct(seq_deserializer([real, imag]))
            }
            DataValue::ComplexDouble { real, imag, .. } => {
                visitor.visit_newtype_struct(seq_deserializer([real, imag]))
            }
            DataValue::VariableLengthArray32 {
                num_elems,
                offset_byte,
            } => visitor.visit_newtype_struct(seq_deserializer([num_elems, offset_byte])),
            DataValue::VariableLengthArray64 {
                num_elems,
                offset_byte,
            } => visitor.visit_newtype_struct(seq_deserializer([num_elems, offset_byte])),
            DataValue::Null => visitor.visit_none(),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: serde::de::Visitor<'de>,
    {
        match self {
            DataValue::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes byte_buf unit
        seq tuple tuple_struct map struct enum identifier ignored_any
        newtype_struct unit_struct
    }
}

impl<'de> IntoDeserializer<'de, Error> for DataValue {
    type Deserializer = Self;

    fn into_deserializer(self) -> Self::Deserializer {
        self
    }
}
