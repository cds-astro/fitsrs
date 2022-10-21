pub use crate::primary_header::PrimaryHeader;

use serde::Serialize;
#[derive(Serialize)]
#[derive(Debug)]
pub struct FitsMemAligned<'a> {
    pub header: PrimaryHeader<'a>,
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
impl<'a> FitsMemAligned<'a> {
    /// Parse a FITS file correctly aligned in memory
    ///
    /// # Arguments
    ///
    /// * `buf` - a slice located at a aligned address location with respect to the type T.
    ///   If T is f32, buf ptr must be divisible by 4
    pub unsafe fn from_byte_slice(buf: &'a [u8]) -> Result<FitsMemAligned<'a>, Error<'a>> {
        let num_total_bytes = buf.len();
        let (buf, header) = PrimaryHeader::new(&buf)?;

        // At this point the header is valid
        let num_pixels = (0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            });

        let num_bytes_consumed = num_total_bytes - buf.len();
        let num_bytes_to_next_line = 80 - num_bytes_consumed % 80;

        let (buf, _) = preceded(
            count(tag(b" "), num_bytes_to_next_line),
            many0(count(tag(b" "), 80)),
        )(buf)?;

        // 1. As the fits file is aligned and data begins at a
        // 80 bytes multiple, then it also begins at a correctly
        // aligned location.
        let x_ptr = buf as *const [u8] as *mut [u8];
        let x_mut_ref = &mut *x_ptr;
        let data = match header.get_bitpix() {
            BitpixValue::U8 => {
                let (_, data, _) = x_mut_ref.align_to_mut::<u8>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                u8::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::U8(data)
            },
            BitpixValue::I16 => {
                let (_, data, _) = x_mut_ref.align_to_mut::<i16>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                i16::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::I16(data)
            },
            BitpixValue::I32 => {
                let (_, data, _) = x_mut_ref.align_to_mut::<i32>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                i32::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::I32(data)
            },
            BitpixValue::I64 => {
                let (_, data, _) = x_mut_ref.align_to_mut::<i64>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                i64::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
                let data = &data[..num_pixels];

                DataTypeBorrowed::I64(data)
            },
            BitpixValue::F32 => {
                let (_, data, _) = x_mut_ref.align_to_mut::<f32>();
                // 2. Convert to big endianness. This is O(N) over the size of the data
                f32::to_slice(data);
                // 3. Keep only the pixels
                assert!(data.len() >= num_pixels);
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

        Ok(FitsMemAligned { header, data })
    }

    pub fn get_header(&'a self) -> &PrimaryHeader<'a> {
        &self.header
    }

    pub fn get_data(&self) -> &DataTypeBorrowed<'_> {
        &self.data
    }
}

#[derive(Serialize)]
#[derive(Debug)]
pub struct FitsMemAlignedUnchecked<'a, T> {
    pub header: PrimaryHeader<'a>,
    pub data: &'a [T],
}

impl<'a, T> FitsMemAlignedUnchecked<'a, T>
where
    T: ToBigEndian
{
    /// Parse a FITS file correctly aligned in memory
    ///
    /// # Arguments
    ///
    /// * `buf` - a slice located at a aligned address location with respect to the type T.
    ///   If T is f32, buf ptr must be divisible by 4
    pub unsafe fn from_byte_slice(buf: &'a [u8]) -> Result<FitsMemAlignedUnchecked<'a, T>, Error<'a>> {
        let num_total_bytes = buf.len();
        let (buf, header) = PrimaryHeader::new(&buf)?;

        // At this point the header is valid
        let num_pixels = (0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            });

        let num_bytes_consumed = num_total_bytes - buf.len();
        let num_bytes_to_next_line = 80 - num_bytes_consumed % 80;

        let (buf, _) = preceded(
            count(tag(b" "), num_bytes_to_next_line),
            many0(count(tag(b" "), 80)),
        )(buf)?;

        // 1. As the fits file is aligned and data begins at a
        // 80 bytes multiple, then it also begins at a correctly
        // aligned location.
        let x_ptr = buf as *const [u8] as *mut [u8];
        let x_mut_ref = &mut *x_ptr;
        
        let (_, data, _) = x_mut_ref.align_to_mut::<T>();
        // 2. Convert to big endianness. This is O(N) over the size of the data
        T::to_slice(data);
        // 3. Keep only the pixels
        assert!(data.len() >= num_pixels);
        let data = &data[..num_pixels];

        Ok(FitsMemAlignedUnchecked { header, data })
    }

    pub fn get_header(&'a self) -> &PrimaryHeader<'a> {
        &self.header
    }

    pub fn get_data(&self) -> &[T] {
        &self.data
    }
}

pub trait ToBigEndian {
    fn read(buf: &[u8]) -> Self;
    fn to_slice(s: &mut [Self]) where Self: Sized;
}

impl ToBigEndian for f64 {
    fn read(buf: &[u8]) -> Self {
        BigEndian::read_f64(buf)
    }

    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_f64(s);
    }
}
impl ToBigEndian for f32 {
    fn read(buf: &[u8]) -> Self {
        BigEndian::read_f32(buf)
    }

    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_f32(s);
    }
}
impl ToBigEndian for i64 {
    fn read(buf: &[u8]) -> Self {
        BigEndian::read_i64(buf)
    }

    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_i64(s);
    }
}
impl ToBigEndian for i32 {
    fn read(buf: &[u8]) -> Self {
        BigEndian::read_i32(buf)
    }

    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_i32(s);
    }
}
impl ToBigEndian for i16 {
    fn read(buf: &[u8]) -> Self {
        BigEndian::read_i16(buf)
    }

    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_i16(s);
    }
}
impl ToBigEndian for u8 {
    fn read(buf: &[u8]) -> Self {
        buf[0]
    }

    fn to_slice(_s: &mut [Self]) {}
}

#[cfg(test)]
mod tests {
    use super::FitsMemAligned;
    use super::DataTypeBorrowed;
    use std::io::Read;

    #[test]
    fn test_fits_f32() {
        use std::fs::File;
        use std::alloc::{dealloc, alloc};
        use std::alloc::Layout;

        // 1. Read the file
        let mut f = File::open("misc/Npix208.fits").unwrap();
        let mut raw_bytes = Vec::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        // 2. Copy the fits file raw bytes to an aligned memory location
        let layout = Layout::from_size_align(raw_bytes.len(), std::mem::size_of::<f32>())
            .expect("Cannot create sized aligned memory layout");
        unsafe {
            let aligned_raw_bytes_ptr = alloc(layout);
            std::ptr::slice_from_raw_parts_mut(aligned_raw_bytes_ptr, raw_bytes.len())
                .as_mut()
                .unwrap()
                .copy_from_slice(&raw_bytes);        

            // 3 parse the fits considering that it is correctly aligned
            let FitsMemAligned { data: data_aligned, .. } = FitsMemAligned::from_byte_slice(
                std::ptr::slice_from_raw_parts(
                    aligned_raw_bytes_ptr,
                    raw_bytes.len()
                )
                .as_ref()
                .unwrap()
            ).unwrap();
            // 4 use it
            match data_aligned {
                DataTypeBorrowed::F32(_) => {},
                _ => unreachable!(),
            }

            // 5 dealloc this aligned memory space
            dealloc(aligned_raw_bytes_ptr, layout);
        }
    }

    #[test]
    fn test_fits_i16() {
        use std::fs::File;
        use std::alloc::{dealloc, alloc};
        use std::alloc::Layout;

        // 1. Read the file
        let mut f = File::open("misc/Npix4906.fits").unwrap();
        let mut raw_bytes = Vec::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        // 2. Copy the fits file raw bytes to an aligned memory location
        let layout = Layout::from_size_align(raw_bytes.len(), std::mem::size_of::<i16>())
            .expect("Cannot create sized aligned memory layout");
        unsafe {
            let aligned_raw_bytes_ptr = alloc(layout);
            std::ptr::slice_from_raw_parts_mut(aligned_raw_bytes_ptr, raw_bytes.len())
                .as_mut()
                .unwrap()
                .copy_from_slice(&raw_bytes);        

            // 3 parse the fits considering that it is correctly aligned
            let FitsMemAligned { data: data_aligned, .. } = FitsMemAligned::from_byte_slice(
                std::ptr::slice_from_raw_parts(
                    aligned_raw_bytes_ptr,
                    raw_bytes.len()
                )
                .as_ref()
                .unwrap()
            ).unwrap();
            // 4 use it
            match data_aligned {
                DataTypeBorrowed::I16(_) => {},
                _ => unreachable!(),
            }

            // 5 dealloc this aligned memory space
            dealloc(aligned_raw_bytes_ptr, layout);
        }
    }
}
