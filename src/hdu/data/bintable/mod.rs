pub mod data;
pub mod rice;

pub use data::TableData;

use std::fmt::Debug;

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
    },
}
