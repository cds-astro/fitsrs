use std::io::Read;
use std::fmt::Debug;
use std::io::Cursor;
use flate2::read::GzDecoder;

use byteorder::BigEndian;
use serde::Serialize;
use super::Bytes;

use crate::{
    byteorder::ReadBytesExt,
};

/*
impl<'a, T> BigEndianByteIt<'a, T> {
    fn from_bytes(bytes: Bytes<'a>) -> Self {
        let limit = bytes.len();
        Self(BigEndianIt::new(bytes, limit))
    }
}*/

/*
/// Impl a seek method that will call seek on the cursor of bytes.
use std::io::{Seek, SeekFrom};
impl<'a, T> BigEndianByteIt<'a, T>
where
    T: Value
{
    pub(crate) fn seek_to_value(&mut self, idx: usize) -> Result<T, Error> {
        let num_bytes_per_type = std::mem::size_of::<T>() as u64;
        let off_bytes = num_bytes_per_type * (idx as u64);
        
        let reader = self.reader();

        reader.seek(SeekFrom::Start(off_bytes))?;

        let v = T::read_be(reader);

        reader.seek(-off_bytes - num_bytes_per_type)?;

        v
    }
}*/

use crate::hdu::Error;
pub(crate) trait Value: Sized {
    fn read_be<R: ReadBytesExt>(reader: &mut R) -> Result<Self, Error>;
}
impl Value for u8 {
    fn read_be<R: ReadBytesExt>(reader: &mut R) -> Result<Self, Error> {
        Ok(reader.read_u8()?)
    }
}
impl Value for i16 {
    fn read_be<R: ReadBytesExt>(reader: &mut R) -> Result<Self, Error> {
        Ok(reader.read_i16::<BigEndian>()?)
    }
}
impl Value for i32 {
    fn read_be<R: ReadBytesExt>(reader: &mut R) -> Result<Self, Error> {
        Ok(reader.read_i32::<BigEndian>()?)
    }
}
impl Value for i64 {
    fn read_be<R: ReadBytesExt>(reader: &mut R) -> Result<Self, Error> {
        Ok(reader.read_i64::<BigEndian>()?)
    }
}
impl Value for f32 {
    fn read_be<R: ReadBytesExt>(reader: &mut R) -> Result<Self, Error> {
        Ok(reader.read_f32::<BigEndian>()?)
    }
}
impl Value for f64 {
    fn read_be<R: ReadBytesExt>(reader: &mut R) -> Result<Self, Error> {
        Ok(reader.read_f64::<BigEndian>()?)
    }
}


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
    pub(crate) fn new(it: I) -> Self {
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

#[derive(Debug, Serialize)]
pub struct BigEndianIt<R, T> {
    /// The reader
    reader: R,
    /// A limit of number of the bytes to read from the reader
    limit: usize,
    /// Number of item read
    cur_idx: usize,
    /// The type of element read from the reader
    _t: std::marker::PhantomData<T>
}

impl<R> BigEndianIt<R, u8>
where
    R: AsRef<[u8]>
{
    fn bytes(&self) -> &[u8] {
        self.reader.as_ref()
    }
}

impl<'a, R, T> BigEndianIt<R, T>
where
    R: Read
{
    pub fn new(reader: R, limit: u64) -> Self {
        Self {
            reader,
            cur_idx: 0,
            limit: limit as usize,
            _t: std::marker::PhantomData,
        }
    }
}

impl<'a, R, T> Iterator for BigEndianIt<R, T>
where
    R: Read,
    T: Value
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.limit == 0 {
            None
        } else {
            let byte = T::read_be(&mut self.reader);
            self.limit -= std::mem::size_of::<T>();
            self.cur_idx += 1;

            byte.ok()
        }
    }
}

use std::io::Seek;
impl<'a, R, T> BigEndianIt<R, T>
where
    R: Read + Seek,
    T: Value
{
    /// Returns the value of the item from a data iterator
    /// 
    /// This internally perform a seek on the inner reader to directly
    /// target the value and it will only read it afterwards
    /// This should be faster than reading the whole stream until the idx
    fn read_value(&mut self, idx: usize) -> Result<T, Error> {
        // Get the position of the reader since the start of the stream
        let t_bytes = std::mem::size_of::<T>() as i64;
        let off = (idx as i64 - self.cur_idx as i64) * t_bytes;

        self.reader.seek_relative(off)?;
        let val = T::read_be(&mut self.reader);
        self.reader.seek_relative(-off - t_bytes)?;

        val
    }
}

