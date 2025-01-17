#[allow(unused_imports)]
use serde::Serialize;

#[allow(unused_imports)]
use super::{iter, Data};
#[allow(unused_imports)]
use super::{stream, AsyncDataBufRead};


pub mod buf;
pub mod cursor;

use std::borrow::Cow;

pub enum FieldTy<'a> {
    // 'L' => Logical
    Logical(Box<[bool]>),
    // 'X' => Bit
    Bit {
        bytes: Cow<'a, [u8]>,
        num_bits: usize,
    },
    // 'B' => Unsigned Byte
    UnsignedByte(Cow<'a, [u8]>),
    // 'I' => 16-bit integer
    Short(Box<[i16]>),
    // 'J' => 32-bit integer
    Integer(Box<[i32]>),
    // 'K' => 64-bit integer
    Long(Box<[i64]>),
    // 'A' => Character
    Character(Cow<'a, [u8]>),
    // 'E' => Single-precision floating point
    Float(Box<[f32]>),
    // 'D' => Double-precision floating point
    Double(Box<[f64]>),
    // 'C' => Single-precision complex
    ComplexFloat(Box<[(f32, f32)]>),
    // 'M' => Double-precision complex
    ComplexDouble(Box<[(f64, f64)]>),
    // 'P' => Array Descriptor (32-bit)
    Array32Desc(Data<'a>),
    // 'Q' => Array Descriptor (64-bit)
    Array64Desc(Data<'a>),
}

pub type Row<'a> = Box<[FieldTy<'a>]>;