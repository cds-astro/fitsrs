use std::io::Read;

use byteorder::BigEndian;
use serde::Serialize;

use crate::{
    byteorder::ReadBytesExt,
    hdu::header::{extension::image::Image, BitpixValue},
};

//use super::Access;

/// An iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
///
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Serialize, Debug)]
pub enum DataIter<'a, R> {
    U8(It<'a, R, u8>),
    I16(It<'a, R, i16>),
    I32(It<'a, R, i32>),
    I64(It<'a, R, i64>),
    F32(It<'a, R, f32>),
    F64(It<'a, R, f64>),
}

impl<'a, R> DataIter<'a, R> {
    pub(crate) fn new(
        ctx: &Image,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut R,
    ) -> Self {
        let bitpix = ctx.get_bitpix();
        match bitpix {
            BitpixValue::U8 => DataIter::U8(It::new(reader, num_remaining_bytes_in_cur_hdu)),
            BitpixValue::I16 => DataIter::I16(It::new(reader, num_remaining_bytes_in_cur_hdu)),
            BitpixValue::I32 => DataIter::I32(It::new(reader, num_remaining_bytes_in_cur_hdu)),
            BitpixValue::I64 => DataIter::I64(It::new(reader, num_remaining_bytes_in_cur_hdu)),
            BitpixValue::F32 => DataIter::F32(It::new(reader, num_remaining_bytes_in_cur_hdu)),
            BitpixValue::F64 => DataIter::F64(It::new(reader, num_remaining_bytes_in_cur_hdu)),
        }
    }
}

#[derive(Serialize, Debug)]
pub struct It<'a, R, T> {
    pub reader: &'a mut R,
    pub num_remaining_bytes_in_cur_hdu: &'a mut usize,
    phantom: std::marker::PhantomData<T>,
}

impl<'a, R, T> It<'a, R, T> {
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
    R: Read,
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
    R: Read,
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
    R: Read,
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
    R: Read,
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
    R: Read,
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
    R: Read,
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
