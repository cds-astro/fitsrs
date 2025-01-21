use byteorder::BigEndian;
#[allow(unused_imports)]
use serde::Serialize;

#[allow(unused_imports)]
use super::{iter, Data};
#[allow(unused_imports)]
use super::{stream, AsyncDataBufRead};


pub mod buf;
pub mod cursor;

use std::borrow::Cow;

#[derive(Debug)]
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
    VariableArray(VariableArray<'a>),
}

pub type Row<'a> = Vec<FieldTy<'a>>;
use flate2::read::GzDecoder;
#[derive(Debug)]
pub enum VariableArray<'a> {
    /// Slice for in-memory readers (cursors), heap allocated array for bufreaders
    U8(Cow<'a, [u8]>),

    /// Iterator over the array converting the bytes to values
    I16(BigEndianIt<Cursor<Cow<'a, [u8]>>, i16>),
    I32(BigEndianIt<Cursor<Cow<'a, [u8]>>, i32>),
    I64(BigEndianIt<Cursor<Cow<'a, [u8]>>, i64>),
    F32(BigEndianIt<Cursor<Cow<'a, [u8]>>, f32>),
    F64(BigEndianIt<Cursor<Cow<'a, [u8]>>, f64>),

    /// GZIP compressed tile images
    GZIP1_U8(
        CastIt<
            BigEndianIt<GzDecoder<Cursor<Cow<'a, [u8]>>>, i32>,
            u8
        >),
    GZIP1_I16(
        CastIt<
            BigEndianIt<GzDecoder<Cursor<Cow<'a, [u8]>>>, i32>,
            i16
        >),
    GZIP1_I32(BigEndianIt<GzDecoder<Cursor<Cow<'a, [u8]>>>, i32>),
}

#[derive(Debug)]
pub struct CastIt<I, T> {
    it: I,
    _t: std::marker::PhantomData<T>
}

impl<I, T> CastIt<I, T> {
    fn new(it: I) -> Self {
        Self {
            it,
            _t: std::marker::PhantomData
        }
    }
}

impl<I> Iterator for CastIt<I, u8>
where
    I: Iterator<Item = i32>
{
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(|v| v as u8)
    }
}

impl<I> Iterator for CastIt<I, i16>
where
    I: Iterator<Item = i32>
{
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next().map(|v| v as i16)
    }
}

#[derive(Debug)]
pub struct BigEndianIt<R, T>
where
    R: Read
{
    reader: R,
    _t: std::marker::PhantomData<T>
}

impl<'a, R, T> BigEndianIt<R, T>
where
    R: Read
{
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            _t: std::marker::PhantomData,
        }
    }
}

use std::io::{Read, Cursor};
use crate::byteorder::ReadBytesExt;
impl<'a, R> Iterator for BigEndianIt<R, u8>
where
    R: Read,
{
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_u8().ok()
    }
}
impl<'a, R> Iterator for BigEndianIt<R, i16>
where
    R: Read,
{
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_i16::<BigEndian>().ok()
    }
}
impl<'a, R> Iterator for BigEndianIt<R, i32>
where
    R: Read,
{
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_i32::<BigEndian>().ok()
    }
}
impl<'a, R> Iterator for BigEndianIt<R, i64>
where
    R: Read,
{
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_i64::<BigEndian>().ok()
    }
}
impl<'a, R> Iterator for BigEndianIt<R, f32>
where
    R: Read,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_f32::<BigEndian>().ok()
    }
}
impl<'a, R> Iterator for BigEndianIt<R, f64>
where
    R: Read,
{
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_f64::<BigEndian>().ok()
    }
}

