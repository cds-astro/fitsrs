use crate::hdu;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::Header;
use crate::hdu::header::Xtension;
use crate::card::Card;

use serde::Serialize;
use std::fmt::Debug;

#[derive(Debug, Serialize)]
pub struct Fits<R> {
    start: bool,
    // Store the number of bytes that remains to read so that the current HDU data finishes
    // When getting the next HDU, we will first consume those bytes if there are some
    num_remaining_bytes_in_cur_hdu: usize,
    // Keep track of the number of total bytes for the current HDU as we might
    // skip the trailing bytes to get to a multiple of 2880 bytes
    num_bytes_in_cur_hdu: usize,
    // If an error has been encountered, the HDU iterator ends
    error_parsing_encountered: bool,
    reader: R,
}
use std::io::Read;
impl<R> Read for Fits<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

use crate::error::Error;
use flate2::bufread::GzDecoder;
use std::fs::File;
use std::io::{BufReader, Seek};
use std::path::Path;
impl<'a, R> Fits<R> {
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a reader created i.e. from the opening of a file
    pub fn from_reader(reader: R) -> Self {
        Self {
            reader,
            num_remaining_bytes_in_cur_hdu: 0,
            num_bytes_in_cur_hdu: 0,
            error_parsing_encountered: false,
            start: true,
        }
    }
}

use hdu::data::DataRead;
impl<'a, R> Fits<R>
where
    R: DataRead<'a, Image> + DataRead<'a, AsciiTable> + DataRead<'a, BinTable> + 'a,
{
    /// Consume the bytes until the next HDU
    ///
    /// 1. If the data has not all been read, then read it
    /// 2. Once all the data has been read we must read the last bytes until a 2880 block of bytes
    /// has been read
    ///     a/ it is possible that EOF is reached immediately because some fits files do not have these blank bytes
    ///        at the end of its last HDU
    ///     b/
    pub(crate) fn consume_until_next_hdu(&mut self) -> Result<(), Error> {
        let mut block_mem_buf: [u8; 2880] = [0; 2880];

        // 1. Check if there are still bytes to be read to get to the end of data
        if self.num_remaining_bytes_in_cur_hdu > 0 {
            // Then read them
            match self
                .reader
                .read_exact(&mut block_mem_buf[..self.num_remaining_bytes_in_cur_hdu])
            {
                Err(e) => return Err(Error::Io(e)),
                Ok(()) => {}
            }
        }

        // 2. We are at the end of the real data. As FITS standard stores data in block of 2880 bytes
        // we must read until the next block of data to get the location of the next HDU

        let is_remaining_bytes = (self.num_bytes_in_cur_hdu % 2880) > 0;
        // Skip the remaining bytes to set the reader where a new HDU begins
        if is_remaining_bytes {
            let num_off_bytes = (2880 - (self.num_bytes_in_cur_hdu % 2880)) as usize;
            match self.reader.read_exact(&mut block_mem_buf[..num_off_bytes]) {
                // An error like unexpected EOF is not permitted by the standard but we make it pass
                // interpreting it as the last HDU in the file
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(()),
                Err(e) => Err(Error::Io(e)),
                Ok(()) => Ok(()),
            }
        } else {
            Ok(())
        }
    }

    // Retrieve the iterator or in memory data from the reader
    // This has the effect of consuming the HDU
    pub fn get_data<X>(&'a mut self, hdu: HDU<X>) -> <R as DataRead<'a, X>>::Data
    where
        X: Xtension + Debug,
        R: DataRead<'a, X> + 'a,
    {
        // Unroll the internal fits parsing parameters to give it to the data reader
        let Self {
            num_remaining_bytes_in_cur_hdu,
            reader,
            ..
        } = self;
        let xtension = hdu.header.get_xtension();
        <R as DataRead<'a, X>>::init_data_reading_process(
            xtension,
            num_remaining_bytes_in_cur_hdu,
            reader,
        )
    }
}

impl<'a, R> Iterator for Fits<R>
where
    R: DataRead<'a, Image> + DataRead<'a, AsciiTable> + DataRead<'a, BinTable> + 'a,
{
    type Item = Result<hdu::HDU, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.error_parsing_encountered {
            None
        } else {
            let n = if !self.start {
                // We must consume the bytes until the next header is found
                // if eof then the iterator finishes
                match self.consume_until_next_hdu() {
                    Ok(()) => {
                        let mut num_bytes_read = 0;
                        match hdu::HDU::new_xtension(&mut self.reader, &mut num_bytes_read) {
                                Ok(hdu) => Some(Ok(hdu)),
                                Err(Error::Io(e))
                                    // an EOF has been encountered but the number of bytes read is 0
                                    // this is valid since we have terminated the previous HDU
                                    if e.kind() == std::io::ErrorKind::UnexpectedEof && num_bytes_read == 0 => {
                                        None
                                    },
                                Err(e) => Some(Err(e))
                            }
                    }
                    Err(e) => Some(Err(e)),
                }
            } else {
                // primary HDU parsing
                let hdu = hdu::HDU::new_primary(&mut self.reader);
                Some(hdu)
            };

            self.start = false;

            match n {
                Some(Ok(hdu)) => {
                    self.num_bytes_in_cur_hdu = match &hdu {
                        hdu::HDU::XImage(h) | hdu::HDU::Primary(h) => {
                            let xtension = h.get_header().get_xtension();
                            xtension.get_num_bytes_data_block() as usize
                        }
                        hdu::HDU::XASCIITable(h) => {
                            let xtension = h.get_header().get_xtension();
                            xtension.get_num_bytes_data_block() as usize
                        }
                        hdu::HDU::XBinaryTable(h) => {
                            let xtension = h.get_header().get_xtension();
                            xtension.get_num_bytes_data_block() as usize
                        }
                    };

                    self.num_remaining_bytes_in_cur_hdu = self.num_bytes_in_cur_hdu;

                    Some(Ok(hdu))
                }
                Some(Err(e)) => {
                    // an error has been found we return it and ends the iterator for future next calls
                    self.error_parsing_encountered = true;

                    Some(Err(e))
                }
                None => None,
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct HDU<X>
where
    X: Xtension,
{
    /// The header part that stores all the cards
    header: Header<X>,
}

impl<X> HDU<X>
where
    X: Xtension + std::fmt::Debug,
{
    pub fn new<'a, R>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        cards: Vec<Card>,
    ) -> Result<Self, Error>
    where
        R: DataRead<'a, X> + 'a,
    {
        let header = Header::parse(cards)?;
        /* 2. Skip the next bytes to a new 2880 multiple of bytes
        This is where the data block should start */
        let is_remaining_bytes = ((*num_bytes_read) % 2880) > 0;

        // Skip the remaining bytes to set the reader where a new HDU begins
        if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = (2880 - ((*num_bytes_read) % 2880)) as usize;
            reader
                .read_exact(&mut block_mem_buf[..num_off_bytes])
                .map_err(|_| Error::StaticError("EOF reached"))?;

            *num_bytes_read += num_off_bytes;
        }

        Ok(Self { header })
    }

    pub fn get_header(&self) -> &Header<X> {
        &self.header
    }
}
