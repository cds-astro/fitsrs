use std::io::BufRead;

use serde::Serialize;

use crate::hdu::data::DataBufRead;
use crate::hdu::extension::XtensionHDU;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::extension::Xtension;
use crate::hdu::header::Header;

//use crate::hdu::data::{DataAsyncBufRead, DataBufRead};

#[derive(Debug, Serialize)]
pub struct Fits<R> {
    start: bool,
    num_bytes_read: usize,
    error_parsing_encountered: bool,
    reader: R,
}

use crate::error::Error;
use crate::hdu::primary::PrimaryHDU;
impl<'a, R> Fits<R> {
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a reader created i.e. from the opening of a file
    pub fn from_reader(reader: R) -> Result<Self, Error> {
        //let hdu = PrimaryHDU::new(reader)?;

        Ok(Self {
            reader,
            num_bytes_read: 0,
            error_parsing_encountered: false,
            start: true,
        })
    }
}

impl<R> Fits<R>
where
    R: BufRead,
{
    /// Returns a boolean to know if we are at EOF
    fn consume_until_next_hdu(&mut self) -> Result<bool, Error> {
        let is_remaining_bytes = (self.num_bytes_read % 2880) > 0;
        // Skip the remaining bytes to set the reader where a new HDU begins
        if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = (2880 - (self.num_bytes_read % 2880)) as usize;
            match self.reader.read_exact(&mut block_mem_buf[..num_off_bytes]) {
                // An error like unexpected EOF is not permitted by the standard but we make it pass
                // interpreting it as the last HDU in the file
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(true),
                Err(e) => return Err(e.into()),
                Ok(()) => {}
            }

            self.num_bytes_read += dbg!(num_off_bytes);
        }

        let eof = self.reader.fill_buf()?.is_empty();
        Ok(eof)
    }
}

impl<'a, R> Iterator for Fits<R>
where
    //R: DataBufRead<'a, Image> + DataBufRead<'a, BinTable> + DataBufRead<'a, AsciiTable> + 'a,
    R: DataBufRead<'a, Image> + 'a,
{
    type Item = Result<XtensionHDU, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.error_parsing_encountered {
            None
        } else if self.start {
            self.start = false;

            if let Ok(prim_hdu) = PrimaryHDU::new(&mut self.reader, &mut self.num_bytes_read) {
                Some(Ok(XtensionHDU::Image(prim_hdu.0)))
            } else {
                None
            }
        } else {
            // We must consume the bytes until the next header is found
            // if eof then the iterator finishes
            match self.consume_until_next_hdu() {
                Ok(eof) => {
                    if dbg!(eof) {
                        None
                    } else {
                        Some(XtensionHDU::new(&mut self.reader, &mut self.num_bytes_read))
                    }
                }
                // an error has been found we return it and ends the iterator for future next calls
                Err(e) => {
                    self.error_parsing_encountered = true;

                    Some(Err(e))
                }
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
    // We do not store the the data part. it will be given when the data are asked by the user
    //data: <R as DataBufRead<'a, X>>::Data,
}

impl<X> HDU<X>
where
    X: Xtension + std::fmt::Debug,
{
    pub fn new<'a, R>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error>
    where
        R: DataBufRead<'a, X> + 'a,
    {
        /* 1. Parse the header first */
        let header = Header::parse(reader, num_bytes_read, card_80_bytes_buf)?;
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

        // Data block

        Ok(Self { header })
    }

    pub fn get_header(&self) -> &Header<X> {
        &self.header
    }

    // Retrieve the iterator or in memory data from the reader
    // This has the effect of consuming the HDU
    pub fn get_data<'a, R>(self, hdu_list: &'a mut Fits<R>) -> <R as DataBufRead<'a, X>>::Data
    where
        R: DataBufRead<'a, X> + DataBufRead<'a, Image>, //+ DataBufRead<'a, BinTable>
                                                        //+ DataBufRead<'a, AsciiTable>,
    {
        // Unroll the internal fits parsing parameters to give it to the data reader
        let Fits {
            num_bytes_read,
            reader,
            ..
        } = hdu_list;
        let xtension = self.header.get_xtension();
        <R as DataBufRead<'a, X>>::prepare_data_reading(xtension, num_bytes_read, reader)
    }
}

/*
#[derive(Debug)]
pub struct AsyncFits<'a, R>
where
    R: DataAsyncBufRead<'a, Image>
        + DataAsyncBufRead<'a, BinTable>
        + DataAsyncBufRead<'a, AsciiTable>,
{
    pub hdu: AsyncPrimaryHDU<'a, R>,
}

impl<'a, R> AsyncFits<'a, R>
where
    R: DataAsyncBufRead<'a, Image>
        + DataAsyncBufRead<'a, BinTable>
        + DataAsyncBufRead<'a, AsciiTable>,
{
    /// Parse a FITS file
    /// # Params
    /// * `reader` - a reader created i.e. from the opening of a file
    pub async fn from_reader(reader: &'a mut R) -> Result<AsyncFits<'a, R>, Error> {
        let hdu = AsyncPrimaryHDU::new(reader).await?;

        Ok(Self { hdu })
    }

    pub fn get_primary_hdu(self) -> AsyncPrimaryHDU<'a, R> {
        self.hdu
    }

    pub async fn get_xtension_hdu(self, mut idx: usize) -> Result<AsyncXtensionHDU<'a, R>, Error> {
        let mut hdu_ext = self.hdu.next().await?;
        if idx == 0 {
            if let Some(hdu) = hdu_ext {
                Ok(hdu)
            } else {
                Err(Error::StaticError("No more ext HDU found"))
            }
        } else {
            while let Some(hdu) = hdu_ext {
                hdu_ext = hdu.next().await?;
                idx -= 1;

                if idx == 0 {
                    break;
                }
            }

            if let Some(hdu) = hdu_ext {
                Ok(hdu)
            } else {
                Err(Error::StaticError("No more ext HDU found"))
            }
        }
    }
}*/
