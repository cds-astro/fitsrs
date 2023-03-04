pub mod header;

pub mod data;
pub mod extension;
pub mod primary;

use header::Header;
use crate::hdu::data::DataBufRead;
use crate::hdu::header::extension::Xtension;

#[derive(Debug)]
pub struct HDU<'a, R, X>
where
    X: Xtension,
    R: DataBufRead<'a, X>
{
    /// The header part that stores all the cards
    header: Header<X>,
    /// The data part
    data: <R as DataBufRead<'a, X>>::Data,
}

use crate::error::Error;
impl<'a, R, X> HDU<'a, R, X>
where
    X: Xtension + std::fmt::Debug,
    R: DataBufRead<'a, X> + 'a
{
    pub fn new(reader: &'a mut R, num_bytes_read: &mut usize, card_80_bytes_buf: &mut [u8; 80]) -> Result<Self, Error> {
        /* 1. Parse the header first */
        let header = Header::parse(reader, num_bytes_read, card_80_bytes_buf)?;
        /* 2. Skip the next bytes to a new 2880 multiple of bytes
        This is where the data block should start */
        let is_remaining_bytes = ((*num_bytes_read) % 2880) > 0;

        // Skip the remaining bytes to set the reader where a new HDU begins
        if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = 2880 - ((*num_bytes_read) % 2880);
            reader.read_exact(&mut block_mem_buf[..num_off_bytes])
                .map_err(|_| Error::StaticError("EOF reached"))?;
        }

        // Data block
        let xtension = dbg!(header.get_xtension());
        let data = reader.new_data_block(xtension);

        Ok(Self {
            header,
            data
        })
    }

    fn consume(self) -> Result<Option<&'a mut R>, Error> {
        let mut num_bytes_read = 0;
        let reader = <R as DataBufRead<'a, X>>::consume_data_block(self.data, &mut num_bytes_read)?;

        let is_remaining_bytes = (num_bytes_read % 2880) > 0;
        // Skip the remaining bytes to set the reader where a new HDU begins
        let reader = if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = 2880 - (num_bytes_read % 2880);
            reader.read_exact(&mut block_mem_buf[..num_off_bytes])
                .ok() // An error like unexpected EOF is not standard frendly but we make it pass
                // interpreting it as the last HDU in the file
                .map(|_| reader)
        } else {
            // We are at a multiple of 2880 byte
            Some(reader)
        };
        
        if let Some(reader) = reader {
            let is_eof = reader.fill_buf().map_err(|_| Error::StaticError("Unable to fill the buffer to check if data is remaining"))?.is_empty();
            if !is_eof {
                Ok(Some(reader))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    pub fn get_header(&self) -> &Header<X> {
        &self.header
    }

    pub fn get_data(&mut self) -> &mut <R as DataBufRead<'a, X>>::Data {
        &mut self.data
    }
}

mod tests {
    use crate::hdu::primary::PrimaryHDU;
    use super::header::BitpixValue;
    use std::io::{Cursor, Read, BufReader};
    use std::fs::File;
    /*
    #[test]
    fn test_cursor_lifetime() {
        let mut f = File::open("misc/Npix208.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();
        // Here all the file content is in memory
        let mut reader = Cursor::new(&raw_bytes[..]);
        let hdu = HDU::new(&mut reader).unwrap();

        assert_eq!(hdu.header.get_bitpix(), BitpixValue::F32);
    }

    #[test]
    fn test_file_lifetime() {
        let f = File::open("misc/Npix208.fits").unwrap();
        let mut reader = BufReader::new(f);

        let hdu = {
            HDU::new(&mut reader).unwrap()
        };

        assert_eq!(hdu.header.get_bitpix(), BitpixValue::F32);
    }*/
}