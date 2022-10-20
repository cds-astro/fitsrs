extern crate nom;

extern crate byteorder;

mod card;
mod error;
mod primary_header;

mod fits;
pub use fits::{FitsMemAligned, ToBigEndian};

pub use card::FITSCardValue;
pub use primary_header::FITSCard;
pub use primary_header::PrimaryHeader;
pub use primary_header::BitpixValue;

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
use std::pin::Pin;
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

#[cfg(test)]
mod tests {
    use crate::FitsMemAligned;

    use super::primary_header::{BitpixValue, FITSCard};
    use super::{PrimaryHeader};
    use std::io::Read;
    #[test]
    fn test_fits_tile() {
        use std::fs::File;
        let f = File::open("misc/Npix208.fits").unwrap();
        let bytes: Result<Vec<_>, _> = f.bytes().collect();
        let buf = bytes.unwrap();
        let FitsMemAligned { header, .. } = unsafe { FitsMemAligned::from_byte_slice(&buf).unwrap() };
        let PrimaryHeader { cards, .. } = header;

        let cards_expect = vec![
            ("SIMPLE", FITSCard::Simple),
            ("BITPIX", FITSCard::Bitpix(BitpixValue::F32)),
            ("NAXIS", FITSCard::Naxis(2)),
            (
                "NAXIS1",
                FITSCard::NaxisSize {
                    name: "NAXIS1",
                    idx: 1,
                    size: 64,
                },
            ),
            (
                "NAXIS2",
                FITSCard::NaxisSize {
                    name: "NAXIS2",
                    idx: 2,
                    size: 64,
                },
            ),
        ];
        assert_eq!(cards, cards_expect);
        println!("{:?}", cards);
    }

    #[test]
    fn test_fits_tile2() {
        use std::fs::File;

        let mut f = File::open("misc/Npix282.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let FitsMemAligned { data, .. } = unsafe { FitsMemAligned::from_byte_slice(&buf[..]).unwrap() };

        match data {
            crate::fits::DataTypeBorrowed::F32(_) => {}
            _ => (),
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

        let _fits = unsafe { FitsMemAligned::from_byte_slice(&buf[..]).unwrap() };
    }

    #[test]
    fn test_fits_tile4() {
        use std::fs::File;

        let mut f = File::open("misc/Npix9.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = unsafe { FitsMemAligned::from_byte_slice(&buf[..]).unwrap() };
    }

    #[test]
    fn test_fits_image() {
        use std::fs::File;

        let mut f = File::open("misc/cutout-CDS_P_HST_PHAT_F475W.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        println!("fsdfsd");

        let _fits = unsafe { FitsMemAligned::from_byte_slice(&buf[..]).unwrap() };
    }

    #[test]
    fn test_fits_image2() {
        use std::fs::File;

        let mut f = File::open("misc/FOCx38i0101t_c0f.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let FitsMemAligned { data, header } = unsafe { FitsMemAligned::from_byte_slice(&buf[..]).unwrap() };

        let naxis1 = header.get_axis_size(0).unwrap();
        let naxis2 = header.get_axis_size(1).unwrap();

        match data {
            crate::fits::DataTypeBorrowed::F32(data) => {
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

        let _fits = unsafe { FitsMemAligned::from_byte_slice(&buf[..]).unwrap() };
    }
    #[test]
    fn test_fits_tile6() {
        use std::fs::File;

        let mut f = File::open("misc/Npix8.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = unsafe { FitsMemAligned::from_byte_slice(&buf[..]).unwrap() };
    }

    #[test]
    fn test_fits_tile7() {
        use std::fs::File;

        let mut f = File::open("misc/allsky_panstarrs.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = unsafe { FitsMemAligned::from_byte_slice(&buf[..]).unwrap() };
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
        unsafe {
            assert!(FitsMemAligned::from_byte_slice(bytes).is_err());            
        }
    }
}
