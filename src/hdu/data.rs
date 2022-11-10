use std::io::{BufRead, Cursor, BufReader, Read};
use byteorder::{BigEndian, ReadBytesExt};

use serde::Serialize;

enum Data<'a, R>
where
    R: BufRead
{
    Borrowed(DataBorrowed<'a>),
    Owned(DataOwned<R>),
}

trait DataRead: BufRead {
    fn new<'a>(reader: Self) -> Data<'a, Self>;
}

impl<R> DataRead for Cursor<R>
where
    R: AsRef<[u8]>
{
    fn new<'a, T>(reader: Self) -> Data<'a, Self> {
        let buf = reader.into_inner();
        let bytes = reader.as_ref();

        Data::Borrowed(DataBorrowed::U8(bytes))
    }
}

impl DataRead for &[u8] {
    fn new<'a>(reader: Self) -> Data<'a, Self> {
        let bytes = reader.as_ref();
        
        Data::Borrowed(DataBorrowed::U8(bytes))
    }
}

impl<R> DataRead for BufReader<R>
where
    R: Read
{
    fn new<'a>(reader: Self) -> Data<'a, Self> {

        Data::Owned(DataOwned::U8(DataOwnedIt { reader: (), phantom: () }))
    }
}

#[derive(Serialize)]
#[derive(Debug)]
pub enum DataBorrowed<'a> {
    U8(&'a [u8]),
    I16(&'a [i16]),
    I32(&'a [i32]),
    I64(&'a [i64]),
    F32(&'a [f32]),
    F64(&'a [f64]),
}

pub enum DataOwned<R>
where
    R: BufRead
{
    U8(DataOwnedIt<R, u8>),
    I16(DataOwnedIt<R, i16>),
    I32(DataOwnedIt<R, i32>),
    I64(DataOwnedIt<R, i64>),
    F32(DataOwnedIt<R, f32>),
    F64(DataOwnedIt<R, f64>),
}
struct DataOwnedIt<R, T>
where
    R: BufRead
{
    reader: R,
    phantom: std::marker::PhantomData<T>,
}

impl<R> Iterator for DataOwnedIt<R, u8>
where
    R: BufRead
{
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_u8().ok()
    }
}

impl<R> Iterator for DataOwnedIt<R, i16>
where
    R: BufRead
{
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_i16::<BigEndian>().ok()
    }
}

impl<R> Iterator for DataOwnedIt<R, i32>
where
    R: BufRead
{
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_i32::<BigEndian>().ok()
    }
}

impl<R> Iterator for DataOwnedIt<R, i64>
where
    R: BufRead
{
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_i64::<BigEndian>().ok()
    }
}

impl<R> Iterator for DataOwnedIt<R, f32>
where
    R: BufRead
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_f32::<BigEndian>().ok()
    }
}

impl<R> Iterator for DataOwnedIt<R, f64>
where
    R: BufRead
{
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        self.reader.read_f64::<BigEndian>().ok()
    }
}
