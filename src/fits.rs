pub use crate::primary_header::PrimaryHeader;

use serde::Serialize;
#[derive(Serialize)]
#[derive(Debug)]
pub struct FitsMemAligned<'a, T>
where
    T: BigEndianSlice
{
    pub header: PrimaryHeader<'a>,
    pub data: &'a [T],
}

use crate::Error;
use nom::bytes::complete::tag;
use nom::multi::{count, many0};
use nom::sequence::preceded;
use byteorder::BigEndian;
use crate::byteorder::ByteOrder;
impl<'a, T> FitsMemAligned<'a, T>
where
    T: BigEndianSlice
{
    /// Parse a FITS file correctly aligned in memory
    ///
    /// # Arguments
    ///
    /// * `buf` - a slice located at a aligned address location with respect to the type T.
    ///   If T is f32, buf ptr must be divisible by 4
    pub unsafe fn from_byte_slice(buf: &'a [u8]) -> Result<FitsMemAligned<'a, T>, Error<'a>> {
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

        Ok(FitsMemAligned { header, data })
    }

    pub fn get_header(&'a self) -> &PrimaryHeader<'a> {
        &self.header
    }

    pub fn get_data(&self) -> &[T] {
        &self.data
    }
}

pub trait BigEndianSlice {
    fn to_slice(s: &mut [Self]) where Self: Sized;
}

impl BigEndianSlice for f32 {
    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_f32(s);
    }
}
impl BigEndianSlice for i32 {
    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_i32(s);
    }
}
impl BigEndianSlice for i16 {
    fn to_slice(s: &mut [Self]) {
        BigEndian::from_slice_i16(s);
    }
}

#[cfg(test)]
mod tests {
    use super::FitsMemAligned;
    use std::io::Read;
    use crate::Fits;

    #[test]
    fn test_fits_f32() {
        use std::fs::File;
        use std::alloc::{dealloc, alloc};
        use std::alloc::Layout;

        let mut f = File::open("misc/Npix208.fits").unwrap();
        let mut raw_bytes = Vec::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let Fits { data, .. } = Fits::from_byte_slice(&raw_bytes[..]).unwrap();

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
            let FitsMemAligned { data: data_aligned, .. } = FitsMemAligned::<f32>::from_byte_slice(
                std::ptr::slice_from_raw_parts(
                    aligned_raw_bytes_ptr,
                    raw_bytes.len()
                )
                .as_ref()
                .unwrap()
            ).unwrap();
            // 4 use it
            match data {
                crate::DataType::F32(data) => {
                    assert_eq!(data.len(), data_aligned.len());
                    assert_eq!(data_aligned, &data[..]);
                },
                _ => (),
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

        let mut f = File::open("misc/Npix4906.fits").unwrap();
        let mut raw_bytes = Vec::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let Fits { data, .. } = Fits::from_byte_slice(&raw_bytes[..]).unwrap();

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
            let FitsMemAligned { data: data_aligned, .. } = FitsMemAligned::<i16>::from_byte_slice(
                std::ptr::slice_from_raw_parts(
                    aligned_raw_bytes_ptr,
                    raw_bytes.len()
                )
                .as_ref()
                .unwrap()
            ).unwrap();
            // 4 use it
            match data {
                crate::DataType::I16(data) => {
                    assert_eq!(data.len(), data_aligned.len());
                    assert_eq!(data_aligned, &data[..]);
                },
                _ => (),
            }

            // 5 dealloc this aligned memory space
            dealloc(aligned_raw_bytes_ptr, layout);
        }
    }
}