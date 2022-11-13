extern crate nom;
extern crate byteorder;

mod hdu;
mod card;
mod error;
mod fits;

pub use fits::Fits;

pub use card::Value;
pub use card::Card;

struct ParseDataUnit<'a, T> {
    idx: usize,
    num_bytes_per_item: usize,
    num_total_bytes: usize,
    data: &'a [u8],
    val: Option<T>,
}

impl<'a, T> ParseDataUnit<'a, T> {
    fn new(data: &'a [u8], num_items: usize) -> Self {
        let num_bytes_per_item = std::mem::size_of::<T>();
        Self {
            idx: 0,
            num_total_bytes: num_items * num_bytes_per_item,
            num_bytes_per_item,
            data,
            val: None,
        }
    }
}
/*use std::pin::Pin;
use std::task::{Context, Poll};
use futures::stream::Stream;
use futures::stream::StreamExt; // for `next`

impl<'a, T> Stream for ParseDataUnit<'a, T>
where
    T: ToBigEndian + Unpin
{
    type Item = T;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Deserialize row by row.
        if let Some(v) = self.val.take() {
            Poll::Ready(Some(v))
        } else {
            if self.idx < self.num_total_bytes {
                let val = T::read(&self.data[self.idx..]);
                self.idx += self.num_bytes_per_item;
                self.val = Some(val);
    
                Poll::Pending
            } else {
                Poll::Ready(None)
            }
        }
    }
}
*/
#[cfg(test)]
mod tests {
    use crate::{Fits, Value};
    use crate::hdu::header::BitpixValue;
    use crate::hdu::data::{DataOwned, DataBorrowed};

    use std::io::Read;
    use std::io::Cursor;
    use std::fs::File;

    #[test]
    fn test_fits_tile() {
        let f = File::open("misc/Npix208.fits").unwrap();
        let bytes: Result<Vec<_>, _> = f.bytes().collect();
        let buf = bytes.unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let fits = Fits::from_byte_slice(&mut reader).unwrap();

        let primary_header = fits.get_header();
        assert_eq!(primary_header.get_axis_size(1).unwrap(), &64);
        assert_eq!(primary_header.get_axis_size(2).unwrap(), &64);
        assert_eq!(primary_header.get_naxis(), 2);
        assert_eq!(primary_header.get_bitpix(), BitpixValue::F32);
    }

    #[test]
    fn test_fits_f32() {
        let mut f = File::open("misc/Npix282.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let mut reader = Cursor::new(&raw_bytes[..]);
        let fits = Fits::from_byte_slice(&mut reader).unwrap();
        let header = fits.get_header();
        match fits.get_data() {
            DataBorrowed::F32(data) => {
                assert!(data.len() == header.get_axis_size(1).unwrap() * header.get_axis_size(2).unwrap())
            },
            _ => unreachable!(),
        }
    }

    /*#[test]
    fn test_fits_async() {
        use std::fs::File;
        use std::io::BufReader;

        let mut f = File::open("misc/Npix282.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        use futures::executor::LocalPool;

        let mut pool = LocalPool::new();

        // run tasks in the pool until `my_app` completes
        pool.run_until(async {
            let Fits { data, .. } = Fits::from_byte_slice_async(&buf[..]).await.unwrap();

            matches!(data, super::DataType::F32(_));
        });
    }*/

    #[test]
    fn test_fits_tile3() {
        use std::fs::File;

        let mut f = File::open("misc/Npix4906.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let mut reader = Cursor::new(&buf[..]);

        let _fits = Fits::from_byte_slice(&mut reader).unwrap();
    }

    #[test]
    fn test_fits_tile4() {
        use std::fs::File;

        let mut f = File::open("misc/Npix9.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let _fits = Fits::from_byte_slice(&mut reader).unwrap();
    }

    #[test]
    fn test_fits_image() {
        use std::fs::File;

        let mut f = File::open("misc/cutout-CDS_P_HST_PHAT_F475W.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let _fits = Fits::from_byte_slice(&mut reader).unwrap();
    }

    #[test]
    fn test_fits_image_owned() {
        use std::fs::File;
        use std::io::BufReader;

        let f = File::open("misc/FOCx38i0101t_c0f.fits").unwrap();
        let Fits { hdu } = Fits::from_byte_slice(BufReader::new(f)).unwrap();

        let header = &hdu.header;
        let naxis1 = header.get_axis_size(1).unwrap();
        let naxis2 = header.get_axis_size(2).unwrap();

        match hdu.data {
            DataOwned::F32(it) => {
                let data = it.collect::<Vec<_>>();
                assert_eq!(data.len(), naxis1 * naxis2);
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_fits_image_borrowed() {
        use std::fs::File;

        let mut f = File::open("misc/FOCx38i0101t_c0f.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let Fits { hdu } = Fits::from_byte_slice(&buf[..]).unwrap();

        let header = &hdu.header;
        let naxis1 = header.get_axis_size(1).unwrap();
        let naxis2 = header.get_axis_size(2).unwrap();

        match hdu.data {
            DataBorrowed::F32(data) => {
                assert_eq!(data.len(), naxis1 * naxis2);
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_fits_tile5() {
        use std::fs::File;

        let mut f = File::open("misc/Npix133.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        Fits::from_byte_slice(&mut reader).unwrap();
    }
    #[test]
    fn test_fits_tile6() {
        use std::fs::File;

        let mut f = File::open("misc/Npix8.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        Fits::from_byte_slice(&mut reader).unwrap();
    }

    #[test]
    fn test_fits_tile7() {
        use std::fs::File;

        let mut f = File::open("misc/allsky_panstarrs.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        Fits::from_byte_slice(&mut reader).unwrap();
    }

    #[test]
    fn test_bad_bytes() {
        let bytes: &[u8] = &[
            60, 33, 68, 79, 67, 84, 89, 80, 69, 32, 72, 84, 77, 76, 32, 80, 85, 66, 76, 73, 67, 32,
            34, 45, 47, 47, 73, 69, 84, 70, 47, 47, 68, 84, 68, 32, 72, 84, 77, 76, 32, 50, 46, 48,
            47, 47, 69, 78, 34, 62, 10, 60, 104, 116, 109, 108, 62, 60, 104, 101, 97, 100, 62, 10,
            60, 116, 105, 116, 108, 101, 62, 52, 48, 52, 32, 78, 111, 116, 32, 70, 111, 117, 110,
            100, 60, 47, 116, 105, 116, 108, 101, 62, 10, 60, 47, 104, 101, 97, 100, 62, 60, 98,
            111, 100, 121, 62, 10, 60, 104, 49, 62, 78, 111, 116, 32, 70, 111, 117, 110, 100, 60,
            47, 104, 49, 62, 10, 60, 112, 62, 84, 104, 101, 32, 114, 101, 113, 117, 101, 115, 116,
            101, 100, 32, 85, 82, 76, 32, 47, 97, 108, 108, 115, 107, 121, 47, 80, 78, 82, 101,
            100, 47, 78, 111, 114, 100, 101, 114, 55, 47, 68, 105, 114, 52, 48, 48, 48, 48, 47, 78,
            112, 105, 120, 52, 52, 49, 49, 49, 46, 102, 105, 116, 115, 32, 119, 97, 115, 32, 110,
            111, 116, 32, 102, 111, 117, 110, 100, 32, 111, 110, 32, 116, 104, 105, 115, 32, 115,
            101, 114, 118, 101, 114, 46, 60, 47, 112, 62, 10, 60, 47, 98, 111, 100, 121, 62, 60,
            47, 104, 116, 109, 108, 62, 10,
        ];
        let mut reader = Cursor::new(bytes);
        assert!(Fits::from_byte_slice(&mut reader).is_err());
    }
}
