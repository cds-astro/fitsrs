use std::io::BufRead;

use byteorder::BigEndian;
use serde::Serialize;

use crate::byteorder::ReadBytesExt;

//use super::Access;

/// An iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Serialize, Debug)]
pub enum DataIter<'a, R>
where
    R: BufRead,
{
    U8(It<'a, R, u8>),
    I16(It<'a, R, i16>),
    I32(It<'a, R, i32>),
    I64(It<'a, R, i64>),
    F32(It<'a, R, f32>),
    F64(It<'a, R, f64>),
}

#[derive(Serialize, Debug)]
pub struct It<'a, R, T>
where
    R: BufRead,
{
    pub reader: &'a mut R,
    pub num_remaining_bytes_in_cur_hdu: &'a mut usize,
    phantom: std::marker::PhantomData<T>,
}

impl<'a, R, T> It<'a, R, T>
where
    R: BufRead,
{
    pub fn new(reader: &'a mut R, num_remaining_bytes_in_cur_hdu: &'a mut usize) -> Self {
        Self {
            reader,
            num_remaining_bytes_in_cur_hdu,
            phantom: std::marker::PhantomData,
        }
    }
}

impl<'a, R> Iterator for It<'a, R, u8>
where
    R: BufRead,
{
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
            None
        } else {
            let item = self.reader.read_u8();

            let num_bytes_item = std::mem::size_of::<Self::Item>();
            *self.num_remaining_bytes_in_cur_hdu -= num_bytes_item;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for It<'a, R, i16>
where
    R: BufRead,
{
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
            None
        } else {
            let item = self.reader.read_i16::<BigEndian>();

            let num_bytes_item = std::mem::size_of::<Self::Item>();
            *self.num_remaining_bytes_in_cur_hdu -= num_bytes_item;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for It<'a, R, i32>
where
    R: BufRead,
{
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
            None
        } else {
            let item = self.reader.read_i32::<BigEndian>();

            let num_bytes_item = std::mem::size_of::<Self::Item>();
            *self.num_remaining_bytes_in_cur_hdu -= num_bytes_item;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for It<'a, R, i64>
where
    R: BufRead,
{
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
            None
        } else {
            let item = self.reader.read_i64::<BigEndian>();

            let num_bytes_item = std::mem::size_of::<Self::Item>();
            *self.num_remaining_bytes_in_cur_hdu -= num_bytes_item;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for It<'a, R, f32>
where
    R: BufRead,
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
            None
        } else {
            let item = self.reader.read_f32::<BigEndian>();

            let num_bytes_item = std::mem::size_of::<Self::Item>();
            *self.num_remaining_bytes_in_cur_hdu -= num_bytes_item;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for It<'a, R, f64>
where
    R: BufRead,
{
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.num_remaining_bytes_in_cur_hdu == 0 {
            None
        } else {
            let item = self.reader.read_f64::<BigEndian>();

            let num_bytes_item = std::mem::size_of::<Self::Item>();
            *self.num_remaining_bytes_in_cur_hdu -= num_bytes_item;

            item.ok()
        }
    }
}

/*
impl<'a, R> Access<'a> for Iter<'a, R, u8>
where
    R: BufRead,
{
    type Type = &'a Self;

    fn get_data(&'a self) -> Self::Type {
        self
    }
}*/
