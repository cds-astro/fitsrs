extern crate nom;
use nom::{bytes::streaming::take, character::complete::multispace0};

extern crate byteorder;
use byteorder::{BigEndian, ByteOrder};

mod card_value;
mod error;
mod primary_header;

use primary_header::PrimaryHeader;
#[derive(Debug)]
pub struct Fits<'a> {
    header: PrimaryHeader<'a>,
    pub data: DataType<'a>,
}

trait ParsingDataUnit: std::marker::Sized {
    fn parse_data_unit(buf: &[u8], num_items: usize) -> Result<DataType, Error> {
        let num_bytes_per_item = std::mem::size_of::<Self>();
        let num_bytes = num_items * num_bytes_per_item;
        let (_, raw_bytes) = take(num_bytes)(buf)?;

        let data = Self::data(raw_bytes, num_items);
        Ok(data)
    }

    fn data(raw_bytes: &[u8], num_items: usize) -> DataType;
}

impl ParsingDataUnit for u8 {
    fn data(raw_bytes: &[u8], _num_items: usize) -> DataType {
        DataType::U8(raw_bytes)
    }
}

impl ParsingDataUnit for i16 {
    fn data(raw_bytes: &[u8], num_items: usize) -> DataType {
        let mut dst: Vec<Self> = vec![0; num_items];
        BigEndian::read_i16_into(raw_bytes, &mut dst);
        DataType::I16(dst)
    }
}

impl ParsingDataUnit for i32 {
    fn data(raw_bytes: &[u8], num_items: usize) -> DataType {
        let mut dst: Vec<Self> = vec![0; num_items];
        BigEndian::read_i32_into(raw_bytes, &mut dst);
        DataType::I32(dst)
    }
}

impl ParsingDataUnit for i64 {
    fn data(raw_bytes: &[u8], num_items: usize) -> DataType {
        let mut dst: Vec<Self> = vec![0; num_items];
        BigEndian::read_i64_into(raw_bytes, &mut dst);
        DataType::I64(dst)
    }
}

impl ParsingDataUnit for f32 {
    fn data(raw_bytes: &[u8], num_items: usize) -> DataType {
        let mut dst: Vec<Self> = vec![0.0; num_items];
        BigEndian::read_f32_into(raw_bytes, &mut dst);
        DataType::F32(dst)
    }
}

impl ParsingDataUnit for f64 {
    fn data(raw_bytes: &[u8], num_items: usize) -> DataType {
        let mut dst: Vec<Self> = vec![0.0; num_items];
        BigEndian::read_f64_into(raw_bytes, &mut dst);
        DataType::F64(dst)
    }
}

use error::Error;
use primary_header::BitpixValue;
impl<'a> Fits<'a> {
    pub fn from_bytes_slice(buf: &'a [u8]) -> Result<Fits<'a>, Error<'a>> {
        let (buf, header) = PrimaryHeader::new(&buf)?;

        // At this point the header is valid
        let num_items = (0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            });

        multispace0(buf)?;

        // Read the byte data stream in BigEndian order conformly to the spec
        let data = match header.get_bitpix() {
            BitpixValue::U8 => u8::parse_data_unit(buf, num_items)?,
            BitpixValue::I16 => i16::parse_data_unit(buf, num_items)?,
            BitpixValue::I32 => i32::parse_data_unit(buf, num_items)?,
            BitpixValue::I64 => i64::parse_data_unit(buf, num_items)?,
            BitpixValue::F32 => f32::parse_data_unit(buf, num_items)?,
            BitpixValue::F64 => f64::parse_data_unit(buf, num_items)?,
        };

        Ok(Fits { header, data })
    }
}

#[derive(Debug)]
pub enum DataType<'a> {
    U8(&'a [u8]),
    I16(Vec<i16>),
    I32(Vec<i32>),
    I64(Vec<i64>),
    F32(Vec<f32>),
    F64(Vec<f64>),
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
        let Fits { header, .. } = Fits::from_bytes_slice(&buf).unwrap();
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
}
