use serde::Serialize;

use crate::card::Card;
use crate::hdu;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::Header;
use crate::hdu::header::Xtension;

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

use crate::error::Error;
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

use hdu::data::DataBufRead;
impl<'a, R> Fits<R>
where
    R: DataBufRead<'a, Image> + DataBufRead<'a, AsciiTable> + DataBufRead<'a, BinTable> + 'a,
{
    /// Returns a boolean to know if we are at EOF
    fn consume_until_next_hdu(&mut self) -> Result<bool, Error> {
        // 1. Check if there are still bytes to be read to get to the end of data
        if self.num_remaining_bytes_in_cur_hdu > 0 {
            // Then read them
            <R as DataBufRead<'_, Image>>::read_n_bytes_exact(
                &mut self.reader,
                self.num_remaining_bytes_in_cur_hdu as u64,
            )?;
        }

        // 2. We are at the end of the real data. As FITS standard stores data in block of 2880 bytes
        // we must read until the next block of data to get the location of the next HDU

        let is_remaining_bytes = (self.num_bytes_in_cur_hdu % 2880) > 0;
        // Skip the remaining bytes to set the reader where a new HDU begins
        if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = (2880 - (self.num_bytes_in_cur_hdu % 2880)) as usize;
            match self.reader.read_exact(&mut block_mem_buf[..num_off_bytes]) {
                // An error like unexpected EOF is not permitted by the standard but we make it pass
                // interpreting it as the last HDU in the file
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(true),
                Err(e) => return Err(e.into()),
                Ok(()) => {}
            }
        }

        let eof = self.reader.fill_buf()?.is_empty();
        Ok(eof)
    }
}

impl<'a, R> Iterator for Fits<R>
where
    R: DataBufRead<'a, Image> + DataBufRead<'a, AsciiTable> + DataBufRead<'a, BinTable> + 'a,
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
                    Ok(eof) => {
                        if !eof {
                            Some(hdu::HDU::new_xtension(&mut self.reader))
                        } else {
                            None
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

#[derive(Debug)]
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
        R: DataBufRead<'a, X> + 'a,
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

    // Retrieve the iterator or in memory data from the reader
    // This has the effect of consuming the HDU
    pub fn get_data<'a, R>(self, hdu_list: &'a mut Fits<R>) -> <R as DataBufRead<'a, X>>::Data
    where
        R: DataBufRead<'a, X> + 'a,
    {
        // Unroll the internal fits parsing parameters to give it to the data reader
        let Fits {
            num_remaining_bytes_in_cur_hdu,
            reader,
            ..
        } = hdu_list;
        let xtension = self.header.get_xtension();
        <R as DataBufRead<'a, X>>::prepare_data_reading(
            xtension,
            num_remaining_bytes_in_cur_hdu,
            reader,
        )
    }
}
