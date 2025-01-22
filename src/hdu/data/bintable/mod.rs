use byteorder::BigEndian;

pub mod buf;
pub mod cursor;

use std::borrow::Cow;

use std::io::{Read, Cursor};
use crate::byteorder::ReadBytesExt;

use crate::hdu::header::extension::bintable::BinTable;

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

impl<'a> FieldTy<'a> {
    fn parse_variable_array(array_bytes: Cow<'a, [u8]>, elem_ty: char, _ctx: &BinTable) -> Self {
        #[cfg(feature="tci")]
        let field = {
            use crate::hdu::header::Bitpix;
            use crate::hdu::header::extension::bintable::ZCmpType;

            // TILE compressed convention case. More details here: https://fits.gsfc.nasa.gov/registry/tilecompression.html
            if let Some(z_image) = &_ctx.z_image {
                let tile_raw_bytes = array_bytes;

                match (z_image.z_cmp_type, z_image.z_bitpix) {
                    // It can only store integer typed values i.e. bytes, short or integers
                    (ZCmpType::Gzip1, Bitpix::U8) => VariableArray::U8(EitherIt::second(tile_raw_bytes.into())),
                    (ZCmpType::Gzip1, Bitpix::I16) => VariableArray::I16(EitherIt::second(tile_raw_bytes.into())),
                    (ZCmpType::Gzip1, Bitpix::I32) => VariableArray::I32(EitherIt::second(tile_raw_bytes.into())),
                    // Return the slice of bytes
                    _ => VariableArray::U8(EitherIt::first(tile_raw_bytes.into()))
                }
            } else {
                // once we have the bytes, convert it accordingly to the type given by the array descriptor
                match elem_ty {
                    'I' => VariableArray::I16(EitherIt::first(array_bytes.into())),
                    'J' => VariableArray::I32(EitherIt::first(array_bytes.into())),
                    'K' => VariableArray::I64(array_bytes.into()),
                    'E' => VariableArray::F32(array_bytes.into()),
                    'D' => VariableArray::F64(array_bytes.into()),
                    _ => VariableArray::U8(EitherIt::first(array_bytes.into())),
                }
            }
        };
        #[cfg(not(feature="tci"))]
        let field = {
            // once we have the bytes, convert it accordingly
            match elem_ty {
                'I' => VariableArray::I16(EitherIt::first(array_bytes.into())),
                'J' => VariableArray::I32(EitherIt::first(array_bytes.into())),
                'K' => VariableArray::I64(array_bytes.into()),
                'E' => VariableArray::F32(array_bytes.into()),
                'D' => VariableArray::F64(array_bytes.into()),
                _ => VariableArray::U8(EitherIt::first(array_bytes.into())),
            }
        };

        FieldTy::VariableArray(field)
    }
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

//#[cfg(feature="tci")]
/// This contains variable array content
#[derive(Debug)]
pub enum VariableArray<'a> {
    /// Iterator that will return bytes and decode the data for tile compressed images
    U8(EitherIt<
        BigEndianByteIt<'a, u8>,
        BigEndianGzByteIt<'a, u8>,
        u8
    >),
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

