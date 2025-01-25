use std::io::Read;
use std::fmt::Debug;

use byteorder::BigEndian;
use serde::Serialize;

use byteorder::ReadBytesExt;

use crate::hdu::Error;
pub trait Value: Sized {
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
    pub fn bytes(&self) -> &[u8] {
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
    pub fn read_value(&mut self, idx: usize) -> Result<T, Error> {
        // Get the position of the reader since the start of the stream
        let t_bytes = std::mem::size_of::<T>() as i64;
        let off = (idx as i64 - self.cur_idx as i64) * t_bytes;

        self.reader.seek_relative(off)?;
        let val = T::read_be(&mut self.reader);
        self.reader.seek_relative(-off - t_bytes)?;

        val
    }
}