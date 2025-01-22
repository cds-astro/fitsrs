use byteorder::BigEndian;

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

/// A type that can store a borrowed or owned slice of bytes
pub type Bytes<'a> = Cow<'a, [u8]>;
/// A type which is a reader over borrowed or owned bytes
type BytesReader<'a> = Cursor<Bytes<'a>>;
/// A Gzip1 decoder reader over a byte reader
type GzBytesReader<'a> = GzDecoder<BytesReader<'a>>;

/// An iterator generating T typed values by reading raw bytes in the big endian scheme.
#[derive(Debug)]
pub struct BigEndianByteIt<'a, T>(BigEndianIt<BytesReader<'a>, T>);
impl<'a, T> From<Bytes<'a>> for BigEndianByteIt<'a, T> {
    fn from(value: Bytes<'a>) -> Self {
        Self(BigEndianIt::new(Cursor::new(value)))
    }
}

impl<'a, T> Iterator for BigEndianByteIt<'a, T>
where
    BigEndianIt<BytesReader<'a>, T>: Iterator
{
    type Item = <BigEndianIt<BytesReader<'a>, T> as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// An iterator generating T typed values by reading gzipped bytes and converting them in the BE scheme.
#[derive(Debug)]
pub struct BigEndianGzByteIt<'a, T>(CastIt<BigEndianIt<GzBytesReader<'a>, i32>, T>);
impl<'a, T> From<Bytes<'a>> for BigEndianGzByteIt<'a, T> {
    fn from(value: Bytes<'a>) -> Self {
        Self(CastIt::new(BigEndianIt::new(GzDecoder::new(Cursor::new(value)))))
    }
}

impl<'a, T> Iterator for BigEndianGzByteIt<'a, T>
where
    CastIt<BigEndianIt<GzBytesReader<'a>, i32>, T>: Iterator
{
    type Item = <CastIt<BigEndianIt<GzBytesReader<'a>, i32>, T> as Iterator>::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}


#[derive(Debug)]
pub enum VariableArray<'a> {
    /// Iterator that will return bytes and decode the data
    /// for tile compressed images
    U8(EitherIt<
        BigEndianByteIt<'a, u8>,
        BigEndianGzByteIt<'a, u8>,
        u8
    >),
    /// Iterator that will return shorts and decode the data
    /// for tile compressed images
    I16(EitherIt<
        BigEndianByteIt<'a, i16>,
        BigEndianGzByteIt<'a, i16>,
        i16
    >),
    /// Iterator that will return integers and decode the data
    /// for tile compressed images
    I32(EitherIt<
        BigEndianByteIt<'a, i32>,
        BigEndianGzByteIt<'a, i32>,
        i32
    >),
    I64(BigEndianByteIt<'a, i64>),
    F32(BigEndianByteIt<'a, f32>),
    F64(BigEndianByteIt<'a, f64>),
}

use std::fmt::Debug;
#[derive(Debug)]
pub enum EitherIt<I, J, T>
where
    I: IntoIterator<Item = T>,
    <I as IntoIterator>::IntoIter: Debug,
    J: IntoIterator<Item = T>,
    <J as IntoIterator>::IntoIter: Debug,
{
    I {
        i: <I as IntoIterator>::IntoIter,
        _t: std::marker::PhantomData<T>
    },
    J {
        j: <J as IntoIterator>::IntoIter,
        _t: std::marker::PhantomData<T>
    }
}

impl<I, J, T> EitherIt<I, J, T>
where
    I: IntoIterator<Item = T>,
    <I as IntoIterator>::IntoIter: Debug,
    J: IntoIterator<Item = T>,
    <J as IntoIterator>::IntoIter: Debug,
{
    pub(crate) fn first(it1: I) -> Self {
        EitherIt::I { i: it1.into_iter(), _t: std::marker::PhantomData }
    }

    pub(crate) fn second(it2: J) -> Self {
        EitherIt::J { j: it2.into_iter(), _t: std::marker::PhantomData }
    }
}

impl<I, J, T> Iterator for EitherIt<I, J, T>
where
    I: IntoIterator<Item = T>,
    <I as IntoIterator>::IntoIter: Debug,
    J: IntoIterator<Item = T>,
    <J as IntoIterator>::IntoIter: Debug,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            EitherIt::I { i, .. } => i.next(),
            EitherIt::J { j, .. } => j.next()
        }
    }
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

impl<I> Iterator for CastIt<I, i32>
where
    I: Iterator<Item = i32>
{
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        self.it.next()
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

