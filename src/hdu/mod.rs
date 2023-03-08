pub mod header;

pub mod data;
pub mod extension;
pub mod primary;

use crate::hdu::data::DataBufRead;
use crate::hdu::header::extension::Xtension;
use futures::{AsyncBufReadExt, AsyncReadExt};
use header::Header;

use self::data::{Access, DataAsyncBufRead};
use crate::error::Error;

#[derive(Debug)]
pub struct HDU<'a, R, X>
where
    X: Xtension,
    R: DataBufRead<'a, X>,
{
    /// The header part that stores all the cards
    header: Header<X>,
    /// The data part
    data: <R as DataBufRead<'a, X>>::Data,
}

impl<'a, R, X> HDU<'a, R, X>
where
    X: Xtension + std::fmt::Debug,
    R: DataBufRead<'a, X> + 'a,
{
    pub fn new(
        reader: &'a mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<Self, Error> {
        /* 1. Parse the header first */
        let header = Header::parse(reader, num_bytes_read, card_80_bytes_buf)?;
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
        }

        // Data block
        let xtension = header.get_xtension();
        let data = reader.new_data_block(xtension);

        Ok(Self { header, data })
    }

    fn consume(self) -> Result<Option<&'a mut R>, Error> {
        let mut num_bytes_read = 0;
        let reader = <R as DataBufRead<'a, X>>::consume_data_block(self.data, &mut num_bytes_read)?;

        let is_remaining_bytes = (num_bytes_read % 2880) > 0;
        // Skip the remaining bytes to set the reader where a new HDU begins
        let reader = if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = 2880 - (num_bytes_read % 2880);
            reader
                .read_exact(&mut block_mem_buf[..num_off_bytes])
                .ok() // An error like unexpected EOF is not standard frendly but we make it pass
                // interpreting it as the last HDU in the file
                .map(|_| reader)
        } else {
            // We are at a multiple of 2880 byte
            Some(reader)
        };

        if let Some(reader) = reader {
            let is_eof = reader
                .fill_buf()
                .map_err(|_| {
                    Error::StaticError("Unable to fill the buffer to check if data is remaining")
                })?
                .is_empty();
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

    pub fn get_data(&self) -> &<<R as DataBufRead<'a, X>>::Data as Access>::Type {
        self.data.get_data()
    }

    pub fn get_data_mut(&mut self) -> &mut <<R as DataBufRead<'a, X>>::Data as Access>::Type {
        self.data.get_data_mut()
    }
}

// Async variant
#[derive(Debug)]
pub struct AsyncHDU<'a, R, X>
where
    X: Xtension,
    R: DataAsyncBufRead<'a, X>,
{
    /// The header part that stores all the cards
    header: Header<X>,
    /// The data part
    data: <R as DataAsyncBufRead<'a, X>>::Data,
}

impl<'a, R, X> AsyncHDU<'a, R, X>
where
    X: Xtension + std::fmt::Debug,
    R: DataAsyncBufRead<'a, X> + std::marker::Send + 'a,
{
    pub async fn new(
        reader: &'a mut R,
        num_bytes_read: &mut usize,
        card_80_bytes_buf: &mut [u8; 80],
    ) -> Result<AsyncHDU<'a, R, X>, Error> {
        /* 1. Parse the header first */
        let header = Header::parse_async(reader, num_bytes_read, card_80_bytes_buf).await?;
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
        }

        // Data block
        let xtension = header.get_xtension();
        let data = reader.new_data_block(xtension);

        Ok(Self { header, data })
    }

    async fn consume(self) -> Result<Option<&'a mut R>, Error> {
        let mut num_bytes_read = 0;
        let reader =
            <R as DataAsyncBufRead<'a, X>>::consume_data_block(self.data, &mut num_bytes_read)
                .await?;

        let is_remaining_bytes = (num_bytes_read % 2880) > 0;
        // Skip the remaining bytes to set the reader where a new HDU begins
        let reader = if is_remaining_bytes {
            let mut block_mem_buf: [u8; 2880] = [0; 2880];

            let num_off_bytes = 2880 - (num_bytes_read % 2880);
            reader
                .read_exact(&mut block_mem_buf[..num_off_bytes])
                .await
                .ok() // An error like unexpected EOF is not standard frendly but we make it pass
                // interpreting it as the last HDU in the file
                .map(|_| reader)
        } else {
            // We are at a multiple of 2880 byte
            Some(reader)
        };

        if let Some(reader) = reader {
            let is_eof = reader
                .fill_buf()
                .await
                .map_err(|_| {
                    Error::StaticError("Unable to fill the buffer to check if data is remaining")
                })?
                .is_empty();
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

    pub fn get_data(&self) -> &<<R as DataAsyncBufRead<'a, X>>::Data as Access>::Type {
        self.data.get_data()
    }

    pub fn get_data_mut(&mut self) -> &mut <<R as DataAsyncBufRead<'a, X>>::Data as Access>::Type {
        self.data.get_data_mut()
    }
}
