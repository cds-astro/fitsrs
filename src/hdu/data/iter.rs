use std::io::BufRead;

use byteorder::BigEndian;
use serde::Serialize;

use crate::byteorder::ReadBytesExt;

use super::Access;

/// An iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Serialize, Debug)]
pub enum Data<'a, R>
where
    R: BufRead,
{
    U8(Iter<'a, R, u8>),
    I16(Iter<'a, R, i16>),
    I32(Iter<'a, R, i32>),
    I64(Iter<'a, R, i64>),
    F32(Iter<'a, R, f32>),
    F64(Iter<'a, R, f64>),
}

impl<'a, R> Access for Data<'a, R>
where
    R: BufRead,
{
    type Type = Self;

    fn get_data(&self) -> &Self::Type {
        self
    }

    fn get_data_mut(&mut self) -> &mut Self::Type {
        self
    }
}

#[derive(Serialize, Debug)]
pub struct Iter<'a, R, T>
where
    R: BufRead,
{
    pub reader: &'a mut R,
    pub num_bytes_to_read: u64,
    pub num_bytes_read: u64,
    phantom: std::marker::PhantomData<T>,
}

impl<'a, R, T> Iter<'a, R, T>
where
    R: BufRead,
{
    pub fn new(reader: &'a mut R, num_bytes_to_read: u64) -> Self {
        Self {
            reader,
            num_bytes_read: 0,
            num_bytes_to_read,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, R> Iterator for Iter<'a, R, u8>
where
    R: BufRead,
{
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_u8();
            self.num_bytes_read += std::mem::size_of::<Self::Item>() as u64;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for Iter<'a, R, i16>
where
    R: BufRead,
{
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_i16::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>() as u64;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for Iter<'a, R, i32>
where
    R: BufRead,
{
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_i32::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>() as u64;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for Iter<'a, R, i64>
where
    R: BufRead,
{
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_i64::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>() as u64;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for Iter<'a, R, f32>
where
    R: BufRead,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_f32::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>() as u64;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for Iter<'a, R, f64>
where
    R: BufRead,
{
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_bytes_to_read == self.num_bytes_read {
            None
        } else {
            let item = self.reader.read_f64::<BigEndian>();
            self.num_bytes_read += std::mem::size_of::<Self::Item>() as u64;

            item.ok()
        }
    }
}

impl<'a, R> Access for Iter<'a, R, u8>
where
    R: BufRead,
{
    type Type = Self;

    fn get_data(&self) -> &Self::Type {
        self
    }

    fn get_data_mut(&mut self) -> &mut Self::Type {
        self
    }
}
