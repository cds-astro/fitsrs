use std::io::{BufRead, Cursor, BufReader, Read};
use byteorder::{BigEndian, ReadBytesExt, ByteOrder};

use serde::Serialize;

use super::header::BitpixValue;

use std::fmt::Debug;

/// Abstraction for reading a data block
pub trait DataRead<'a>: BufRead {
    type Data: Debug;

    unsafe fn read_data_block(&'a mut self, bitpix: BitpixValue, num_pixels: usize) -> Self::Data where Self: Sized;
}

impl<'a, R> DataRead<'a> for Cursor<R>
where
    R: AsRef<[u8]> + 'a
{
    type Data = DataBorrowed<'a>;

    unsafe fn read_data_block(&mut self, bitpix: BitpixValue, num_pixels: usize) -> Self::Data {
        let bytes = self.get_ref();
        let bytes = bytes.as_ref();

        let pos = self.position() as usize;

        let bytes = &bytes[pos..];
        let x_ptr = bytes as *const [u8] as *mut [u8];
        let x_mut_ref = &mut *x_ptr;

        match bitpix {
            BitpixValue::U8 => {
                let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
                debug_assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                let num_bytes = num_pixels;
                self.consume(num_bytes);

                DataBorrowed::U8(data)
            },
            BitpixValue::I16 => {
                // 1. Verify the alignement
                let (_, data, _) = x_mut_ref.align_to_mut::<i16>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                BigEndian::from_slice_i16(data);
                // 3. Keep only the pixels
                debug_assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                let num_bytes = num_pixels * std::mem::size_of::<i16>();
                self.consume(num_bytes);

                DataBorrowed::I16(data)
            },
            BitpixValue::I32 => {
                // 1. Verify the alignement
                let (_, data, _) = x_mut_ref.align_to_mut::<i32>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                BigEndian::from_slice_i32(data);
                // 3. Keep only the pixels
                debug_assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                let num_bytes = num_pixels * std::mem::size_of::<i32>();
                self.consume(num_bytes);

                DataBorrowed::I32(data)
            },
            BitpixValue::I64 => {
                // 1. Verify the alignement
                let (_, data, _) = x_mut_ref.align_to_mut::<i64>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                BigEndian::from_slice_i64(data);
                // 3. Keep only the pixels
                debug_assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                let num_bytes = num_pixels * std::mem::size_of::<i64>();
                self.consume(num_bytes);

                DataBorrowed::I64(data)
            },
            BitpixValue::F32 => {
                // 1. Verify the alignement
                let (_, data, _) = x_mut_ref.align_to_mut::<f32>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                BigEndian::from_slice_f32(data);
                // 3. Keep only the pixels
                debug_assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                let num_bytes = num_pixels * std::mem::size_of::<f32>();
                self.consume(num_bytes);

                DataBorrowed::F32(data)
            },
            BitpixValue::F64 => {
                // 1. Verify the alignement
                let (_, data, _) = x_mut_ref.align_to_mut::<f64>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                BigEndian::from_slice_f64(data);
                // 3. Keep only the pixels
                debug_assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                let num_bytes = num_pixels * std::mem::size_of::<f64>();
                self.consume(num_bytes);

                DataBorrowed::F64(data)
            }
        }
    }
}

impl<'a, R> DataRead<'a> for BufReader<R>
where
    R: Read + Debug + 'a
{
    type Data = DataOwned<'a, Self>;

    unsafe fn read_data_block(&'a mut self, bitpix: BitpixValue, num_pixels: usize) -> Self::Data {
        match bitpix {
            BitpixValue::U8 => DataOwned::U8(DataOwnedIt::new(self, num_pixels)),
            BitpixValue::I16 => DataOwned::I16(DataOwnedIt::new(self, num_pixels)),
            BitpixValue::I32 => DataOwned::I32(DataOwnedIt::new(self, num_pixels)),
            BitpixValue::I64 => DataOwned::I64(DataOwnedIt::new(self, num_pixels)),
            BitpixValue::F32 => DataOwned::F32(DataOwnedIt::new(self, num_pixels)),
            BitpixValue::F64 => DataOwned::F64(DataOwnedIt::new(self, num_pixels)),
        }
    }
}

/// The full slice of data found in-memory
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
/// 
/// The data part is expressed as a `DataBorrowed` structure
/// for in-memory readers (typically for `&[u8]` or a `Cursor<AsRef<[u8]>>`) that ensures
/// all the data fits in memory
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

/// An iterator on the data array
/// This is an enum whose content depends on the
/// bitpix value found in the header part of the HDU
/// 
/// The data part is expressed as a `DataOwned` structure
/// for non in-memory readers (typically BufReader) that ensures
/// a file may not fit in memory
#[derive(Serialize)]
#[derive(Debug)]
pub enum DataOwned<'a, R>
where
    R: BufRead
{
    U8(DataOwnedIt<'a, R, u8>),
    I16(DataOwnedIt<'a, R, i16>),
    I32(DataOwnedIt<'a, R, i32>),
    I64(DataOwnedIt<'a, R, i64>),
    F32(DataOwnedIt<'a, R, f32>),
    F64(DataOwnedIt<'a, R, f64>),
}

#[derive(Serialize)]
#[derive(Debug)]
pub struct DataOwnedIt<'a, R, T>
where
    R: BufRead
{
    reader: &'a mut R,
    num_pixels: usize,
    counter: usize,
    phantom: std::marker::PhantomData<T>,
}

impl<'a, R, T> DataOwnedIt<'a, R, T>
where
    R: BufRead
{
    fn new(reader: &'a mut R, num_pixels: usize) -> Self {
        let counter = 0;
        Self {
            reader,
            counter,
            num_pixels,
            phantom: std::marker::PhantomData
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, u8>
where
    R: BufRead
{
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_pixels == self.counter {
            None
        } else {
            //let item = self.reader.read_u8();
            let item = self.reader.read_u8();
            self.counter += 1;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, i16>
where
    R: BufRead
{
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_pixels == self.counter {
            None
        } else {
            let item = self.reader.read_i16::<BigEndian>();
            self.counter += 1;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, i32>
where
    R: BufRead
{
    type Item = i32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_pixels == self.counter {
            None
        } else {
            let item = self.reader.read_i32::<BigEndian>();
            self.counter += 1;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, i64>
where
    R: BufRead
{
    type Item = i64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_pixels == self.counter {
            None
        } else {
            let item = self.reader.read_i64::<BigEndian>();
            self.counter += 1;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, f32>
where
    R: BufRead
{
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_pixels == self.counter {
            None
        } else {
            let item = self.reader.read_f32::<BigEndian>();
            self.counter += 1;

            item.ok()
        }
    }
}

impl<'a, R> Iterator for DataOwnedIt<'a, R, f64>
where
    R: BufRead
{
    type Item = f64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.num_pixels == self.counter {
            None
        } else {
            let item = self.reader.read_f64::<BigEndian>();
            self.counter += 1;

            item.ok()
        }
    }
}