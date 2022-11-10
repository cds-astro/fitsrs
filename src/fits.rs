pub use crate::primary_header::PrimaryHeader;

use serde::Serialize;
#[derive(Serialize)]
#[derive(Debug)]
pub struct Fits<'a> {
    pub hdu: PrimaryHeader<'a>,
    pub data: DataTypeBorrowed<'a>,
}

#[derive(Serialize)]
#[derive(Debug)]
pub enum DataTypeBorrowed<'a> {
    U8(&'a [u8]),
    I16(&'a [i16]),
    I32(&'a [i32]),
    I64(&'a [i64]),
    F32(&'a [f32]),
    F64(&'a [f64]),
}

use nom::bytes::complete::tag;
use nom::multi::{count, many0};
use nom::sequence::preceded;
use byteorder::BigEndian;
use crate::byteorder::ByteOrder;
pub use crate::primary_header::BitpixValue;

use crate::error::Error;
use std::io::{BufReader, BufRead, Read, Cursor};
impl<'a> Fits<'a> {
    /// Parse a FITS file correctly aligned in memory
    ///
    /// # Arguments
    ///
    /// * `buf` - a slice located at a aligned address location with respect to the type T.
    ///   If T is f32, buf ptr must be divisible by 4
    pub unsafe fn from_byte_slice<T: AsRef<[u8]> + 'a>(mut reader: Cursor<T>) -> Result<Self, Error> {
        let mut bytes_read = 0;
        let header = dbg!(PrimaryHeader::parse::<T>(&mut reader, &mut bytes_read)?);
        dbg!(bytes_read);
        // At this point the header is valid
        let num_pixels = dbg!((0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            }));

        //let num_bytes_consumed = num_total_bytes - buf.len();
        bytes_read += 2880 - bytes_read % 2880;

        let buffer = reader.into_inner();
        let mut buffer: &[u8] = &buffer.as_ref()[bytes_read..];

        /*loop {
            if let Some(c) = buffer.iter().peekable().next() {
                if *c != b' ' {
                    break;
                } else {
                    buffer.consume(80);
                    bytes_read += 80;
                }
            } else {
                return Err(Error::Not80BytesMultipleFile)
            }
        }*/;

        //let x_mut_ptr = buffer.as_ptr() as *mut u8;
        // 1. As the fits file is aligned and data begins at a
        // 80 bytes multiple, then it also begins at a correctly
        // aligned location.
        let x_ptr = buffer as *const [u8] as *mut [u8];
        let x_mut_ref = &mut *x_ptr;
        let data = match header.get_bitpix() {
            BitpixValue::U8 => {
                // Get a pointer on a [u8; N] where N = N_pixels * sizeof<T>()
                //let x_mut_ref = unsafe { std::slice::from_raw_parts_mut(x_mut_ptr, num_pixels * std::mem::size_of::<u8>()) };
                let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                u8::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::U8(data)
            },
            BitpixValue::I16 => {
                //let x_mut_ref = unsafe { std::slice::from_raw_parts_mut(x_mut_ptr, num_pixels * std::mem::size_of::<i16>()) };
                let (_, data, _) = x_mut_ref.align_to_mut::<i16>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                i16::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::I16(data)
            },
            BitpixValue::I32 => {
                //let x_mut_ref = unsafe { std::slice::from_raw_parts_mut(x_mut_ptr, num_pixels * std::mem::size_of::<i32>()) };
                let (_, data, _) = x_mut_ref.align_to_mut::<i32>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                i32::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::I32(data)
            },
            BitpixValue::I64 => {
                //let x_mut_ref = unsafe { std::slice::from_raw_parts_mut(x_mut_ptr, num_pixels * std::mem::size_of::<i64>()) };
                let (_, data, _) = x_mut_ref.align_to_mut::<i64>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                i64::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::I64(data)
            },
            BitpixValue::F32 => {
                //println!("fsdfsdf");
                let (_, data, _) = x_mut_ref.align_to_mut::<f32>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                f32::to_slice(data);
                // 3. Keep only the pixels
                assert!(dbg!(data.len()) >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::F32(data)
            },
            BitpixValue::F64 => {
                let (_, data, _) = x_mut_ref.align_to_mut::<f64>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                f64::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::F64(data)
            },
        };

        Ok(Self { hdu: header, data })
    }

    pub fn get_header(&'a self) -> &PrimaryHeader<'a> {
        &self.hdu
    }

    pub fn get_data(&self) -> &DataTypeBorrowed<'_> {
        &self.data
    }
}

pub trait ToBigEndian {
    fn to_slice(s: &mut [Self]) where Self: Sized;
}

impl ToBigEndian for f64 {
    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_f64(s);
    }
}
impl ToBigEndian for f32 {
    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_f32(s);
    }
}
impl ToBigEndian for i64 {
    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_i64(s);
    }
}
impl ToBigEndian for i32 {
    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_i32(s);
    }
}
impl ToBigEndian for i16 {
    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_i16(s);
    }
}
impl ToBigEndian for u8 {
    fn to_slice(_s: &mut [Self]) {}
}

#[cfg(test)]
mod tests {
    use super::Fits;
    use super::DataTypeBorrowed;
    use std::io::Read;
    use std::io::BufReader;
    use std::io::Cursor;
    use std::fs::File;

    #[test]
    fn test_fits_f32() {
        let mut f = File::open("misc/Npix208.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let mut reader = Cursor::new(&raw_bytes[..]);
        unsafe {
            let Fits { data, hdu } = Fits::from_byte_slice(reader).unwrap();
            match data {
                DataTypeBorrowed::F32(data) => {
                    assert!(data.len() == hdu.get_axis_size(0).unwrap() * hdu.get_axis_size(1).unwrap())
                },
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_fits_i16() {
        let mut f = File::open("misc/Npix4906.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let mut reader = Cursor::new(&raw_bytes[..]);
        unsafe {
            let Fits { data, hdu } = Fits::from_byte_slice(reader).unwrap();
            match data {
                DataTypeBorrowed::I16(data) => {
                    assert!(data.len() == hdu.get_axis_size(0).unwrap() * hdu.get_axis_size(1).unwrap())
                },
                _ => unreachable!(),
            }
        }
    }
}
