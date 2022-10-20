extern crate nom;
use nom::bytes::complete::take;

extern crate byteorder;
use byteorder::{BigEndian, ByteOrder};

mod card_value;
mod error;
mod primary_header;

mod fits;
pub use fits::{FitsMemAligned, ToBigEndian};

pub use card_value::FITSKeywordValue;
pub use primary_header::FITSHeaderKeyword;
pub use primary_header::PrimaryHeader;
pub use primary_header::BitpixValue;

use serde::Serialize;
#[derive(Serialize)]
#[derive(Debug)]
pub struct Fits<'a> {
    pub header: PrimaryHeader<'a>,
    pub data: DataType<'a>,
}

trait DataUnit<'a>: std::marker::Sized {
    type Item: Default;

    fn parse(buf: &'a [u8], num_items: usize) -> Result<Self, Error<'a>> {
        let num_bytes_per_item = std::mem::size_of::<Self::Item>();
        let num_bytes = num_items * num_bytes_per_item;
        let (_, raw_bytes) = take(num_bytes)(buf)?;

        let data = Self::new(raw_bytes, num_items);
        Ok(data)
    }

    fn new(raw_bytes: &'a [u8], num_items: usize) -> Self;
}

use std::borrow::Cow;
#[derive(Debug)]
#[derive(Serialize)]
pub struct DataUnitU8<'a>(pub Cow<'a, [u8]>);
impl<'a> DataUnit<'a> for DataUnitU8<'a> {
    type Item = u8;
    fn new(raw_bytes: &'a [u8], _num_items: usize) -> Self {
        DataUnitU8(Cow::Borrowed(raw_bytes))
    }
}

#[derive(Debug)]
#[derive(Serialize)]
pub struct DataUnitI16(pub Vec<i16>);
impl<'a> DataUnit<'a> for DataUnitI16 {
    type Item = i16;
    fn new(raw_bytes: &[u8], num_items: usize) -> Self {
        let mut dst: Vec<Self::Item> = vec![Self::Item::default(); num_items];
        BigEndian::read_i16_into(raw_bytes, &mut dst);

        DataUnitI16(dst)
    }
}
impl std::ops::Deref for DataUnitI16 {
    type Target = Vec<i16>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug)]
#[derive(Serialize)]
pub struct DataUnitI32(pub Vec<i32>);
impl<'a> DataUnit<'a> for DataUnitI32 {
    type Item = i32;
    fn new(raw_bytes: &[u8], num_items: usize) -> Self {
        let mut dst: Vec<Self::Item> = vec![Self::Item::default(); num_items];
        BigEndian::read_i32_into(raw_bytes, &mut dst);

        DataUnitI32(dst)
    }
}
impl std::ops::Deref for DataUnitI32 {
    type Target = Vec<i32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[derive(Debug)]
#[derive(Serialize)]
pub struct DataUnitI64(pub Vec<i64>);
impl<'a> DataUnit<'a> for DataUnitI64 {
    type Item = i64;
    fn new(raw_bytes: &[u8], num_items: usize) -> Self {
        let mut dst: Vec<Self::Item> = vec![Self::Item::default(); num_items];
        BigEndian::read_i64_into(raw_bytes, &mut dst);

        DataUnitI64(dst)
    }
}
impl std::ops::Deref for DataUnitI64 {
    type Target = Vec<i64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[derive(Debug)]
#[derive(Serialize)]
pub struct DataUnitF32(pub Vec<f32>);
impl<'a> DataUnit<'a> for DataUnitF32 {
    type Item = f32;
    fn new(raw_bytes: &'a [u8], num_items: usize) -> Self {
        let mut dst: Vec<Self::Item> = vec![Self::Item::default(); num_items];
        BigEndian::read_f32_into(raw_bytes, &mut dst);

        DataUnitF32(dst)
    }
}
impl std::ops::Deref for DataUnitF32 {
    type Target = Vec<f32>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[derive(Debug)]
#[derive(Serialize)]
pub struct DataUnitF64(pub Vec<f64>);
impl<'a> DataUnit<'a> for DataUnitF64 {
    type Item = f64;
    fn new(raw_bytes: &[u8], num_items: usize) -> Self {
        let mut dst: Vec<Self::Item> = vec![Self::Item::default(); num_items];
        BigEndian::read_f64_into(raw_bytes, &mut dst);

        DataUnitF64(dst)
    }
}
impl std::ops::Deref for DataUnitF64 {
    type Target = Vec<f64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

use error::Error;
use nom::bytes::complete::tag;
use nom::multi::{count, many0};
use nom::sequence::preceded;
impl<'a> Fits<'a> {
    pub fn from_byte_slice(buf: &'a [u8]) -> Result<Fits<'a>, Error<'a>> {
        let num_total_bytes = buf.len();
        let (buf, header) = PrimaryHeader::new(&buf)?;

        // At this point the header is valid
        let num_items = (0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            });

        //white_space0(buf)?;
        let num_bytes_consumed = num_total_bytes - buf.len();
        let num_bytes_to_next_line = 80 - num_bytes_consumed % 80;

        let (buf, _) = preceded(
            count(tag(b" "), num_bytes_to_next_line),
            many0(count(tag(b" "), 80)),
        )(buf)?;

        // Read the byte data stream in BigEndian order conformly to the spec
        let data = match header.get_bitpix() {
            BitpixValue::U8 => DataType::U8(DataUnitU8::parse(buf, num_items)?),
            BitpixValue::I16 => DataType::I16(DataUnitI16::parse(buf, num_items)?),
            BitpixValue::I32 => DataType::I32(DataUnitI32::parse(buf, num_items)?),
            BitpixValue::I64 => DataType::I64(DataUnitI64::parse(buf, num_items)?),
            BitpixValue::F32 => DataType::F32(DataUnitF32::parse(buf, num_items)?),
            BitpixValue::F64 => DataType::F64(DataUnitF64::parse(buf, num_items)?),
        };

        Ok(Fits { header, data })
    }

    pub async fn from_byte_slice_async(buf: &'a [u8]) -> Result<Fits<'a>, Error<'a>> {
        let num_total_bytes = buf.len();
        let (buf, header) = PrimaryHeader::new(&buf)?;

        // At this point the header is valid
        let num_items = (0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            });

        //white_space0(buf)?;
        let num_bytes_consumed = num_total_bytes - buf.len();
        let num_bytes_to_next_line = 80 - num_bytes_consumed % 80;

        let (buf, _) = preceded(
            count(tag(b" "), num_bytes_to_next_line),
            many0(count(tag(b" "), 80)),
        )(buf)?;

        // Read the byte data stream in BigEndian order conformly to the spec
        let data = match header.get_bitpix() {
            BitpixValue::U8 => {
                DataType::U8(DataUnitU8(Cow::Borrowed(&buf[..num_items])))
            },
            BitpixValue::I16 => {
                let mut stream = ParseDataUnit::<i16>::new(buf, num_items);
                let mut res = vec![];
                while let Some(item) = stream.next().await {
                    res.push(item);
                }

                DataType::I16(DataUnitI16(res))
            },
            BitpixValue::I32 => {
                let mut stream = ParseDataUnit::<i32>::new(buf, num_items);
                let mut res = vec![];
                while let Some(item) = stream.next().await {
                    res.push(item);
                }

                DataType::I32(DataUnitI32(res))
            },
            BitpixValue::I64 => {
                let mut stream = ParseDataUnit::<i64>::new(buf, num_items);
                let mut res = vec![];
                while let Some(item) = stream.next().await {
                    res.push(item);
                }

                DataType::I64(DataUnitI64(res))
            },
            BitpixValue::F32 => {
                let mut stream = ParseDataUnit::<f32>::new(buf, num_items);
                let mut res = vec![];
                while let Some(item) = stream.next().await {
                    res.push(item);
                }

                DataType::F32(DataUnitF32(res))
            },
            BitpixValue::F64 => {
                let mut stream = ParseDataUnit::<f64>::new(buf, num_items);
                let mut res = vec![];
                while let Some(item) = stream.next().await {
                    res.push(item);
                }

                DataType::F64(DataUnitF64(res))
            }
        };

        Ok(Fits { header, data })
    }

    pub fn get_header(&'a self) -> &PrimaryHeader<'a> {
        &self.header
    }

    pub fn get_data(&'a self) -> &DataType<'a> {
        &self.data
    }
}

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

#[derive(Debug)]
#[derive(Serialize)]
pub enum DataType<'a> {
    U8(DataUnitU8<'a>),
    I16(DataUnitI16),
    I32(DataUnitI32),
    I64(DataUnitI64),
    F32(DataUnitF32),
    F64(DataUnitF64),
}

#[cfg(test)]
mod tests {
    use super::primary_header::{BitpixValue, FITSHeaderKeyword};
    use super::{Fits, PrimaryHeader};
    use std::io::Read;
    #[test]
    fn test_fits_tile() {
        use std::fs::File;
        let f = File::open("misc/Npix208.fits").unwrap();
        let bytes: Result<Vec<_>, _> = f.bytes().collect();
        let buf = bytes.unwrap();
        let Fits { header, .. } = Fits::from_byte_slice(&buf).unwrap();
        let PrimaryHeader { cards, .. } = header;

        let cards_expect = vec![
            ("SIMPLE", FITSHeaderKeyword::Simple),
            ("BITPIX", FITSHeaderKeyword::Bitpix(BitpixValue::F32)),
            ("NAXIS", FITSHeaderKeyword::Naxis(2)),
            (
                "NAXIS1",
                FITSHeaderKeyword::NaxisSize {
                    name: "NAXIS1",
                    idx: 1,
                    size: 64,
                },
            ),
            (
                "NAXIS2",
                FITSHeaderKeyword::NaxisSize {
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

        let Fits { data, .. } = Fits::from_byte_slice(&buf[..]).unwrap();

        match data {
            super::DataType::F32(_) => {}
            _ => (),
        }
    }

    #[test]
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
    }

    #[test]
    fn test_fits_tile3() {
        use std::fs::File;

        let mut f = File::open("misc/Npix4906.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = Fits::from_byte_slice(&buf[..]).unwrap();
    }

    #[test]
    fn test_fits_tile4() {
        use std::fs::File;

        let mut f = File::open("misc/Npix9.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = Fits::from_byte_slice(&buf[..]).unwrap();
    }

    #[test]
    fn test_fits_image() {
        use std::fs::File;

        let mut f = File::open("misc/cutout-CDS_P_HST_PHAT_F475W.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        println!("fsdfsd");

        let _fits = Fits::from_byte_slice(&buf[..]).unwrap();
    }

    /*#[test]
    fn test_fits_image2() {
        use std::fs::File;

        let mut f = File::open("misc/FOCx38i0101t_c0f.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = Fits::from_byte_slice(&buf[..]).unwrap();
    }*/

    #[test]
    fn test_fits_tile5() {
        use std::fs::File;

        let mut f = File::open("misc/Npix133.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = Fits::from_byte_slice(&buf[..]).unwrap();
    }
    #[test]
    fn test_fits_tile6() {
        use std::fs::File;

        let mut f = File::open("misc/Npix8.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = Fits::from_byte_slice(&buf[..]).unwrap();
    }

    #[test]
    fn test_fits_tile7() {
        use std::fs::File;

        let mut f = File::open("misc/allsky_panstarrs.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let _fits = Fits::from_byte_slice(&buf[..]).unwrap();
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
        assert!(Fits::from_byte_slice(bytes).is_err());
    }
}
