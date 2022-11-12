use std::io::{BufRead, Cursor, BufReader, Read};
use byteorder::{BigEndian, ReadBytesExt, ByteOrder};

use serde::Serialize;

use super::header::BitpixValue;

#[derive(Serialize)]
#[derive(Debug)]
pub enum Data<'a, R>
where
    R: BufRead
{
    Borrowed {
        data: DataBorrowed<'a>,
    },
    Owned(DataOwned<R>),
}

pub trait DataRead<'a>: BufRead {
    fn read_data_block(self, bitpix: BitpixValue, num_pixels: usize) -> Data<'a, Self> where Self: Sized;
}

impl<'a, R> DataRead<'a> for Cursor<R>
where
    R: AsRef<[u8]> + 'a
{
    fn read_data_block(mut self, bitpix: BitpixValue, num_pixels: usize) -> Data<'a, Self> {
        //let buf = self.into_inner();
        //let bytes = buf.as_ref();

        // Before returning the parsed data block, consume the Cursor
        unsafe {
            Data::Borrowed { 
                data: DataBorrowed::from_bytes(&self, bitpix, num_pixels),
            }
        }
    }
}

impl<'a, R> DataRead<'a> for BufReader<R>
where
    R: Read
{
    fn read_data_block(self, bitpix: BitpixValue, _num_pixels: usize) -> Data<'a, Self> {
        Data::Owned(match bitpix {
            BitpixValue::U8 => DataOwned::U8(DataOwnedIt::new(self)),
            BitpixValue::I16 => DataOwned::I16(DataOwnedIt::new(self)),
            BitpixValue::I32 => DataOwned::I32(DataOwnedIt::new(self)),
            BitpixValue::I64 => DataOwned::I64(DataOwnedIt::new(self)),
            BitpixValue::F32 => DataOwned::F32(DataOwnedIt::new(self)),
            BitpixValue::F64 => DataOwned::F64(DataOwnedIt::new(self)),
        })
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

impl<'a> DataBorrowed<'a> {
    unsafe fn from_bytes<T>(bytes_buf: &'a Cursor<T>, bitpix: BitpixValue, num_pixels: usize) -> Self
    where
        T: AsRef<[u8]> + 'a
    {
        let bytes_buf = bytes_buf.get_ref();
        let bytes = bytes_buf.as_ref();
        let x_ptr = bytes as *const [u8] as *mut [u8];
        let x_mut_ref = &mut *x_ptr;

        match bitpix {
            BitpixValue::U8 => {
                let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
                debug_assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];
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

                DataBorrowed::F64(data)
            }
        }
    }
}

#[derive(Serialize)]
#[derive(Debug)]
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

#[derive(Serialize)]
#[derive(Debug)]
pub struct DataOwnedIt<R, T>
where
    R: BufRead
{
    reader: R,
    phantom: std::marker::PhantomData<T>,
}

impl<R, T> DataOwnedIt<R, T>
where
    R: BufRead
{
    fn new(reader: R) -> Self {
        Self {
            reader,
            phantom: std::marker::PhantomData
        }
    }
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
