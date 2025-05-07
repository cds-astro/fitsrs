use crate::card::Card;
use crate::hdu;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::Header;
use crate::hdu::header::Xtension;

use std::fmt::Debug;

#[derive(Debug)]
pub struct Fits<R> {
    start: bool,
    // Store the number of bytes that remains to read so that the current HDU data finishes
    // When getting the next HDU, we will first consume those bytes if there are some
    pos_start_cur_du: usize,
    // Keep track of the number of total bytes for the current HDU as we might
    // skip the trailing bytes to get to a multiple of 2880 bytes
    num_bytes_in_cur_du: usize,
    // If an error has been encountered, the HDU iterator ends
    error_parsing_encountered: bool,
    // The reader
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

impl<R> Clone for Fits<R>
where
    R: Clone,
{
    fn clone(&self) -> Self {
        Self {
            reader: self.reader.clone(),
            error_parsing_encountered: self.error_parsing_encountered,
            num_bytes_in_cur_du: self.num_bytes_in_cur_du,
            pos_start_cur_du: self.pos_start_cur_du,
            start: self.start,
        }
    }
}

use crate::error::Error;

impl<R> Fits<R> {
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a reader created i.e. from the opening of a file
    pub fn from_reader(reader: R) -> Self {
        Self {
            reader,
            pos_start_cur_du: 0,
            num_bytes_in_cur_du: 0,
            error_parsing_encountered: false,
            start: true,
        }
    }
}
use hdu::data::FitsRead;
use std::io::Seek;
impl<'a, R> Fits<R>
where
    R: FitsRead<'a, Image> + FitsRead<'a, AsciiTable> + FitsRead<'a, BinTable> + 'a + Seek,
{
    /// Targets the reader to the next HDU
    ///
    /// It is possible that EOF is reached immediately because some fits files do not have these blank bytes
    /// at the end of its last HDU
    pub(crate) fn consume_until_next_hdu(&mut self) -> Result<(), Error> {
        // Seek to the beginning of the next HDU.
        // The number of bytes to skip is the remaining bytes +
        // an offset to get to a multiple of 2880 bytes

        // current seek position since the start of the stream
        let cur_pos = self.reader.stream_position()? as usize;
        let mut num_bytes_to_skip = self.num_bytes_in_cur_du - (cur_pos - self.pos_start_cur_du);

        let offset_in_2880_block = self.num_bytes_in_cur_du % 2880;
        let is_aligned_on_block = offset_in_2880_block == 0;
        if !is_aligned_on_block {
            num_bytes_to_skip += 2880 - offset_in_2880_block;
        }
        self.reader.seek_relative(num_bytes_to_skip as i64)?;

        Ok(())
    }
}

impl<R> Fits<R> {
    /// Get the byte index where the data for the current processed HDU is
    ///
    /// At least one next call has to be done
    pub fn get_position_data_unit(&self) -> usize {
        self.pos_start_cur_du
    }
}

impl<'a, R> Fits<R>
where
    R: FitsRead<'a, Image> + FitsRead<'a, AsciiTable> + FitsRead<'a, BinTable> + 'a,
{
    // Retrieve the iterator or in memory data from the reader
    // This has the effect of consuming the HDU
    pub fn get_data<X>(&'a mut self, hdu: &HDU<X>) -> <R as FitsRead<'a, X>>::Data
    where
        X: Xtension + Debug,
        R: FitsRead<'a, X> + 'a,
    {
        // Unroll the internal fits parsing parameters to give it to the data reader
        let header = &hdu.header;
        self.reader
            .read_data_unit(header, self.pos_start_cur_du as u64)
    }
}

impl<'a, R> Iterator for Fits<R>
where
    R: FitsRead<'a, Image> + FitsRead<'a, AsciiTable> + FitsRead<'a, BinTable> + Debug + 'a + Seek,
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
                                Err(Error::Io(kind))
                                    // an EOF has been encountered but the number of bytes read is 0
                                    // this is valid since we have terminated the previous HDU
                                    if kind == std::io::ErrorKind::UnexpectedEof && num_bytes_read == 0 => {
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
                    self.num_bytes_in_cur_du = hdu.get_data_unit_byte_size() as usize;
                    self.pos_start_cur_du = hdu.get_data_unit_byte_offset() as usize;

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

/// A generic over a HDU type
#[derive(Debug, PartialEq)]
pub struct HDU<X>
where
    X: Xtension,
{
    /// The header part that stores all the cards
    header: Header<X>,
    data_unit_byte_offset: u64,
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
        R: FitsRead<'a, X> + Seek + 'a,
    {
        /* 1. Parse the header first */
        let header = Header::parse(cards)?;
        /* 2. Skip the next bytes to a new 2880 multiple of bytes
        This is where the data block should start */
        let is_remaining_bytes = ((*num_bytes_read) % 2880) > 0;

        // Skip the remaining bytes to set the reader where a new HDU begins
        if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = 2880 - ((*num_bytes_read) % 2880);
            reader
                .read_exact(&mut block_mem_buf[..num_off_bytes])
                .map_err(|_| Error::StaticError("EOF reached"))?;

            *num_bytes_read += num_off_bytes;
        }

        let data_unit_byte_offset = reader.stream_position()?;

        Ok(Self {
            header,
            data_unit_byte_offset,
        })
    }

    pub fn get_header(&self) -> &Header<X> {
        &self.header
    }

    pub fn get_data_unit_byte_offset(&self) -> u64 {
        self.data_unit_byte_offset
    }

    pub fn get_data_unit_byte_size(&self) -> u64 {
        let xtension = self.get_header().get_xtension();
        xtension.get_num_bytes_data_block()
    }
}
