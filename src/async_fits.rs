use std::pin::Pin;

use crate::card::Card;
use crate::hdu;
use crate::hdu::data::AsyncDataBufRead;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::extension::Xtension;
use crate::hdu::header::Header;
use futures::{Future, Stream};
use serde::Serialize;
use std::fmt::Debug;

//use crate::hdu::data::{DataAsyncBufRead, DataBufRead};

#[derive(Debug, Serialize)]
pub struct AsyncFits<R> {
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
impl<R> AsyncFits<R> {
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

use futures::AsyncBufReadExt;
impl<'a, R> AsyncFits<R>
where
    R: AsyncDataBufRead<'a, Image>
        + AsyncDataBufRead<'a, AsciiTable>
        + AsyncDataBufRead<'a, BinTable>
        + 'a,
{
    /// Returns a boolean to know if we are at EOF
    async fn consume_until_next_hdu(&mut self) -> Result<bool, Error> {
        // 1. Check if there are still bytes to be read to get to the end of data
        if self.num_remaining_bytes_in_cur_hdu > 0 {
            // Then read them
            <R as AsyncDataBufRead<'_, Image>>::read_n_bytes_exact(
                &mut self.reader,
                self.num_remaining_bytes_in_cur_hdu as u64,
            )
            .await?;
        }

        // 2. We are at the end of the real data. As FITS standard stores data in block of 2880 bytes
        // we must read until the next block of data to get the location of the next HDU

        let is_remaining_bytes = (self.num_bytes_in_cur_hdu % 2880) > 0;
        // Skip the remaining bytes to set the reader where a new HDU begins
        if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = 2880 - (self.num_bytes_in_cur_hdu % 2880);
            match self
                .reader
                .read_exact(&mut block_mem_buf[..num_off_bytes])
                .await
            {
                // An error like unexpected EOF is not permitted by the standard but we make it pass
                // interpreting it as the last HDU in the file
                Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(true),
                Err(e) => return Err(e.into()),
                Ok(()) => {}
            }
        }

        let eof = self.reader.fill_buf().await?.is_empty();
        Ok(eof)
    }

    pub fn get_data<X>(&'a mut self, hdu: &AsyncHDU<X>) -> <R as AsyncDataBufRead<'a, X>>::Data
    where
        X: Xtension + Debug,
        R: AsyncDataBufRead<'a, X>,
    {
        // Unroll the internal fits parsing parameters to give it to the data reader
        let AsyncFits {
            num_remaining_bytes_in_cur_hdu,
            reader,
            ..
        } = self;
        let xtension = hdu.header.get_xtension();
        <R as AsyncDataBufRead<'a, X>>::prepare_data_reading(
            xtension,
            num_remaining_bytes_in_cur_hdu,
            reader,
        )
    }
}

use futures::task::Context;
use futures::task::Poll;
impl<'a, R> Stream for AsyncFits<R>
where
    R: AsyncDataBufRead<'a, Image>
        + AsyncDataBufRead<'a, BinTable>
        + AsyncDataBufRead<'a, AsciiTable>
        + 'a,
{
    type Item = Result<hdu::AsyncHDU, Error>;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.error_parsing_encountered {
            Poll::Ready(None)
        } else {
            let hdu = if !self.start {
                // We must consume the bytes until the next header is found
                // if eof then the iterator finishes
                let mut consume_tokens_until_next_hdu = self.consume_until_next_hdu();
                let eof = match std::pin::pin!(consume_tokens_until_next_hdu).poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    // The future finished returning the eof information
                    Poll::Ready(r) => r,
                };

                match eof {
                    Ok(eof) => {
                        if !eof {
                            // parse the extension HDU
                            let r = &mut self.reader;
                            let mut parse_x_hdu = hdu::AsyncHDU::new_xtension(r);
                            match std::pin::pin!(parse_x_hdu).poll(cx) {
                                Poll::Pending => return Poll::Pending,
                                // the future finished, returning the parsed hdu or the error while parsing it
                                Poll::Ready(r) => Some(r),
                            }
                        } else {
                            None
                        }
                    }
                    Err(e) => Some(Err(e)),
                }
            } else {
                // parse the primary HDU
                let mut parse_first_hdu = hdu::AsyncHDU::new_primary(&mut self.reader);
                match std::pin::pin!(parse_first_hdu).poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    // the future finished, returning the parsed hdu or the error while parsing it
                    Poll::Ready(r) => Some(r),
                }
            };

            self.start = false;

            match hdu {
                Some(Ok(hdu)) => {
                    self.num_bytes_in_cur_hdu = match &hdu {
                        hdu::AsyncHDU::XImage(h) | hdu::AsyncHDU::Primary(h) => {
                            let xtension = h.get_header().get_xtension();
                            xtension.get_num_bytes_data_block() as usize
                        }
                        hdu::AsyncHDU::XASCIITable(h) => {
                            let xtension = h.get_header().get_xtension();
                            xtension.get_num_bytes_data_block() as usize
                        }
                        hdu::AsyncHDU::XBinaryTable(h) => {
                            let xtension = h.get_header().get_xtension();
                            xtension.get_num_bytes_data_block() as usize
                        }
                    };

                    self.num_remaining_bytes_in_cur_hdu = self.num_bytes_in_cur_hdu;

                    Poll::Ready(Some(Ok(hdu)))
                }
                Some(Err(e)) => {
                    // an error has been found we return it and ends the iterator for future next calls
                    self.error_parsing_encountered = true;

                    Poll::Ready(Some(Err(e)))
                }
                None => Poll::Ready(None),
            }
        }
    }
}

#[derive(Debug)]
pub struct AsyncHDU<X>
where
    X: Xtension,
{
    /// The header part that stores all the cards
    header: Header<X>,
}

use futures::AsyncReadExt;
impl<X> AsyncHDU<X>
where
    X: Xtension + std::fmt::Debug,
{
    pub async fn new<'a, R>(
        reader: &mut R,
        num_bytes_read: &mut usize,
        cards: Vec<Card>,
    ) -> Result<Self, Error>
    where
        R: AsyncDataBufRead<'a, X> + 'a,
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
                .await
                .map_err(|_| Error::StaticError("EOF reached"))?;

            *num_bytes_read += num_off_bytes;
        }

        // Data block

        Ok(Self { header })
    }

    pub fn get_header(&self) -> &Header<X> {
        &self.header
    }
}
