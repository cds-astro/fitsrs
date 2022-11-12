mod header;
mod data;
pub use data::Data;
pub use header::Header;

use std::io::BufRead;
use serde::Serialize;

pub use data::DataRead;
#[derive(Serialize)]
#[derive(Debug)]
pub struct HDU<'a, R>
where
    R: DataRead<'a>
{
    pub header: Header,
    pub data: Data<'a, R>,
}

use crate::error::Error;
impl<'a, R> HDU<'a, R>
where
    R: DataRead<'a>
{
    pub fn new(mut reader: R) -> Result<Self, Error> {
        let mut bytes_read = 0;
        /* 1. Parse the header first */
        let header = dbg!(Header::parse(&mut reader, &mut bytes_read)?);
        dbg!(bytes_read);
        // At this point the header is valid
        let num_pixels = dbg!((0..header.get_naxis())
            .map(|idx| header.get_axis_size(idx).unwrap())
            .fold(1, |mut total, val| {
                total *= val;
                total
            }));
        let bitpix = header.get_bitpix();

        /* 2. Skip the next bytes to a new 2880 multiple of bytes
        This is where the data block should start */
        let off_data_block = 2880 - bytes_read % 2880;
        reader.consume(off_data_block);

        let data = reader.read_data_block(bitpix, num_pixels);

        Ok(Self {
            header,
            data
        })
    }
}