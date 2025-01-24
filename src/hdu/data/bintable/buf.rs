use std::fmt::Debug;
use std::ops::Index;
use crate::hdu::data::{AsyncDataBufRead, stream::St};
use crate::hdu::header::extension::bintable::{P, Q, ArrayDescriptor, L, X, A, B, I, J, K, M, E, C, D, TForm, TileCompressedImage, ZCmpType};

use byteorder::{BigEndian, ByteOrder};
use crate::hdu::header::extension::bintable::{BinTable, TFormType};
use crate::hdu::FitsRead;
use crate::hdu::header::extension::Xtension;
use log::warn;
use std::io::{BufReader, SeekFrom};
use std::io::Read;

impl<'a, R> FitsRead<'a, BinTable> for R
where
    R: Read + Debug + 'a,
{
    type Data = RowIt<R>;

    fn read_data_unit(&mut self, ctx: &BinTable) -> Self::Data
        where
            Self: Sized {
        RowIt::new(self, ctx)
    }
}
/*
#[derive(Debug)]
pub struct RowBytesIt<R> {
    /// The reader over the binary table data
    pub reader: R,
    num_bytes_in_row: usize,

    num_rows: usize,
    idx_row: usize,

    /// A buffer storing the current row bytes
    row_buf: Box<[u8]>,
}


impl<'a, R> RowBytesIt<R> {
    fn new(
        reader: R,
        ctx: &BinTable,
    ) -> Self {
        let num_bytes_in_row = ctx.naxis1 as usize;

        let num_rows: usize = (ctx.naxis1 * ctx.naxis2) as usize / num_bytes_in_row;
        let idx_row = 0;
        let row_buf = vec![0_u8; num_bytes_in_row].into_boxed_slice();

        Self {
            reader,
            num_bytes_in_row,
            idx_row,
            num_rows,
            row_buf
        }
    }
}

impl<'a, R> Iterator for RowBytesIt<R>
where
    R: Read
{
    type Item = Box<[u8]>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx_row == self.num_rows {
            None
        } else {
            // The cursor is always positioned at the beginning of the main table data.
            // i.e. nothing is read until all the data block has been read (with the heap)
            // once it is done the consume_until_hdu method will perform the read
            self.reader.read_exact(&mut self.row_buf).ok()?;

            self.idx_row = self.idx_row + 1;

            Some(self.row_buf.clone())
        }
    }
}
*/

use flate2::read::GzDecoder;

/// A reader that can overload another reader when the necessity comes to
/// 
/// When parsing tile compressed images, we might need to overload the current reader with a Gzip/RICE decoder
#[derive(Debug)]
enum DataReader {
    /// Reader on the main table
    MainTable,
    /// Reader on the HEAP.
    /// FITS tile compressed image convention introduce that tile can be encoded
    /// Therefore we decorate our reader
    HEAP {
        /// The position of the reader before leaving the main table
        /// This will be used to go back to the main table reading state
        main_table_pos: SeekFrom,
        /// The type contained in the heap that we are reading
        ty: char,
        /// The number of bytes remaining to read
        num_bytes_to_read: u64
    }
}
/*
impl<R> Read for DataReader<R>
where
    R: Read
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::MainTable(r) => r.read(buf),
            Self::HEAP { reader , .. } => reader.read(buf)
        }
    }
}
*/
use crate::error::Error;
impl DataReader {
    /// Jump to the heap at a specific offset in the HEAP associated to the binary table
    /// 
    /// This method takes the ownership to change the state of it
    /// 
    /// * Params
    /// 
    /// ctx - The context will give some properties (i.e. location of the heap)
    /// ttype - column name given by the ttype
    /// byte_offset - byte offset from the start of the HEAP region
    /// byte_offset_from_main_table - a byte offset where the reader is in the main table
    fn jump_to_heap<R: Read + Seek>(
        &mut self,
        reader: &mut R,
        ctx: &BinTable,
        ttype: &str,
        byte_offset: u64,
        byte_offset_from_main_table: u64,
        ty: char,
        num_bytes_to_read: u64,
    ) -> Result<(), Error> {
        match *self {
            // We are already in the heap, we do nothing
            DataReader::HEAP { .. } => Ok(()),
            DataReader::MainTable => {
                // Move to the HEAP
                let off =
                    // go back to the beginning of the main table data block
                    - (byte_offset_from_main_table as i64)
                    // from the beginning of the main table go to the beginning of the heap
                    + ctx.theap as i64
                    // from the beginning of the heap go to the start of the array
                    + byte_offset as i64;

                // Get the current location used to go back to the main table
                let pos = SeekFrom::Start(reader.stream_position()?);

                // Go to the HEAP
                let _ = reader.seek_relative(off)?;

                /*let reader = match (ttype.to_uppercase().as_str(), &ctx.z_image) {
                    ("COMPRESSED_DATA", Some(TileCompressedImage { z_cmp_type: ZCmpType::Rice, .. })) => {
                        Decoder::RICEReader(RICEDecoder::new(reader))
                    },
                    ("COMPRESSED_DATA", Some(TileCompressedImage { z_cmp_type: ZCmpType::Gzip1, .. })) => {
                        Decoder::GzipReader(GzDecoder::new(reader))
                    },
                    ("GZIP_COMPRESSED_DATA", _) => Decoder::GzipReader(GzDecoder::new(reader)),
                    _ => reader
                };*/

                *self = DataReader::HEAP { main_table_pos: pos, ty, num_bytes_to_read };

                Ok(())
            }
        }
    }

    fn jump_to_main_table(self) -> Result<Self, Error> {
        Ok(match self {
            // We are already in the heap, we do nothing
            DataReader::HEAP { reader, main_table_pos, ..} => {
                let mut reader = reader.inner();

                // move back to the main table
                let _ = reader.seek(main_table_pos)?;

                DataReader::MainTable(reader)
            },
            DataReader::MainTable(_) => self
        })
    }
}

#[derive(Debug)]
enum Decoder<R> {
    /// No decoding
    Nothing(R),
    /// GZIP1 decoder (Tile compressed image convention)
    GzipReader(GzDecoder<R>),
    /// TODO RICE decoder (Tile compressed)
    RICEReader(RICEDecoder<R>)
}

impl<R> Decoder<R> {
    fn inner(self) -> R {
        match self {
            Decoder::Nothing(reader) => reader,
            Decoder::GzipReader(gz_reader) => gz_reader.into_inner(),
            Decoder::RICEReader(rice_reader) => rice_reader.into_inner()
        }
    }
}


impl<R> Read for Decoder<R>
where
    R: Read
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Nothing(r) => r.read(buf),
            Self::GzipReader(r) => r.read(buf),
            Self::RICEReader(r) => r.read(buf),
        }
    }
}



use byteorder::ReadBytesExt;


#[derive(Debug)]
pub struct RowIt<R> {
    pub reader: DataReader<R>,

    /// Columns ids to return the values, by default all fields are taken into account
    pub cols_idx: Vec<usize>,
    /// A byte offset to seek 
    pub col_byte_offsets: Vec<usize>,
    /// Context of the binary table it contains all the mandatory and optional cards parsed from the header unit
    pub ctx: BinTable,

    /// Col index currently read
    col_idx: usize,
    /// Item index currently read in the cur col read
    item_idx: usize,
    /// A byte offset from the beginning of the main data table to check if we read all the bin table
    byte_offset: usize,
    /// The total number of bytes in the main data table
    main_data_table_byte_size: usize,
}

impl<'a, R> RowIt<R> {
    pub fn new(
        reader: R,
        ctx: &BinTable,
    ) -> Self {
        let reader = DataReader::MainTable(reader);

        // Compute an byte offset for each columns to know at which byte index does the column
        // starts inside a row
        let mut cumul_num_bytes = 0;
        let col_byte_offsets = ctx.tforms
            .iter()
            .map(|tform| {
                let col_byte_offset = cumul_num_bytes;
                cumul_num_bytes += tform.num_bytes_field();
                col_byte_offset
            }).collect();

        let cols_idx = (0..(ctx.tfields)).collect();

        let col_idx = 0;
        let item_idx = 0;

        let main_data_table_byte_size = (ctx.naxis1 * ctx.naxis2) as usize;
        let byte_offset = 0;

        Self {
            reader,
            col_idx,
            item_idx,
            cols_idx,
            col_byte_offsets,
            byte_offset,
            main_data_table_byte_size,
            ctx: ctx.clone()
        }
    }

    /// Select fields to look, by default when not calling this method,
    /// all field values are returned
    pub fn select_fields(&mut self, cols: &[ColumnId]) -> &mut Self {
        self.cols_idx = cols.iter().filter_map(|col| {
            match col {
                ColumnId::Index(index) => Some(*index),
                ColumnId::Name(name) => {
                    // If the column is given by its name, then we must search
                    // in the ttypes keywords to get its correct index
                    match self.ctx.ttypes.iter().position(|ttype| {
                        if let Some(ttype) = ttype {
                            ttype == name
                        } else {
                            false
                        }
                    }) {
                        Some(idx) => Some(idx),
                        None => {
                            warn!("{} field name has not been found. Its value is discarded", name);
                            None
                        }
                    }
                }
            }
        }).collect();

        self
    }
}

use std::io::Cursor;
/// Retrieve the current position of the inner reader for cursor readers
impl<R> RowIt<Cursor<R>> {
    fn position(&self) -> u64 {
        self.reader.position()
    }
}

use std::io::Seek;
use super::rice::RICEDecoder;
use super::{DataValue, ColumnId};
impl<'a, R> Iterator for RowIt<R>
where
    R: Read + Seek + Debug + 'a,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = DataValue;

    fn next(&mut self) -> Option<Self::Item> {
        // Check whether we are at the end of the main data table
        // This would mean we read all the table
        // Read the next value

        // First get the column index in the main data table where the reader is
        let col_idx = self.cols_idx[self.col_idx];

        match &mut self.reader {
            DataReader::MainTable(reader) => {
                if self.byte_offset == self.main_data_table_byte_size {
                    None
                } else {
                    // Retrieve its tform to know which type to read from the reader
                    match &self.ctx.tforms[col_idx] {
                        // Logical
                        TFormType::L { .. } => {
                            let byte = reader.read_u8().ok()?;
                            self.byte_offset += L::BYTES_SIZE;

                            Some(DataValue::Logical { value: byte != 0, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // Bit
                        TFormType::X { .. } => {
                            let byte = reader.read_u8().ok()?;
                            self.byte_offset += X::BYTES_SIZE;

                            Some(DataValue::Bit { byte, bit_idx: 0, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // Unsigned byte
                        TFormType::B { .. } => {
                            let byte = reader.read_u8().ok()?;
                            self.byte_offset += B::BYTES_SIZE;

                            Some(DataValue::UnsignedByte { value: byte, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // 16-bit integer
                        TFormType::I { .. } => {
                            let short = reader.read_i16::<BigEndian>().ok()?;
                            self.byte_offset += I::BYTES_SIZE;

                            Some(DataValue::Short { value: short, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // 32-bit integer
                        TFormType::J { .. } => {
                            let int = reader.read_i32::<BigEndian>().ok()?;
                            self.byte_offset += J::BYTES_SIZE;

                            Some(DataValue::Integer { value: int, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // 64-bit integer
                        TFormType::K { .. } => {
                            let long = reader.read_i64::<BigEndian>().ok()?;
                            self.byte_offset += K::BYTES_SIZE;

                            Some(DataValue::Long { value: long, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // Character
                        TFormType::A { .. } => {
                            let c = reader.read_u8().ok()?;
                            self.byte_offset += A::BYTES_SIZE;

                            Some(DataValue::Character { value: c as char, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // Single-precision floating point
                        TFormType::E { .. } => {
                            let float = reader.read_f32::<BigEndian>().ok()?;
                            self.byte_offset += E::BYTES_SIZE;

                            Some(DataValue::Float { value: float, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // Double-precision floating point
                        TFormType::D { .. } => {
                            let double = reader.read_f64::<BigEndian>().ok()?;
                            self.byte_offset += D::BYTES_SIZE;

                            Some(DataValue::Double { value: double, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // Single-precision complex
                        TFormType::C { .. } => {
                            let real = reader.read_f32::<BigEndian>().ok()?;
                            let imag = reader.read_f32::<BigEndian>().ok()?;
                            self.byte_offset += C::BYTES_SIZE;

                            Some(DataValue::ComplexFloat { real, imag, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // Double-precision complex
                        TFormType::M { .. } => {
                            let real = reader.read_f64::<BigEndian>().ok()?;
                            let imag = reader.read_f64::<BigEndian>().ok()?;
                            self.byte_offset += M::BYTES_SIZE;

                            Some(DataValue::ComplexDouble { real, imag, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                        },
                        // Array Descriptor (32-bit) 
                        TFormType::P { ty, t_byte_size, .. } => {
                            let n_elems = reader.read_u32::<BigEndian>().ok()?;
                            let byte_offset = reader.read_u32::<BigEndian>().ok()?;

                            self.byte_offset += P::BYTES_SIZE;

                            self.reader.jump_to_heap(
                                &self.ctx,
                                "",
                                byte_offset as u64,
                                self.byte_offset as u64,
                                *ty,
                                (n_elems as u64) * (*t_byte_size)
                            ).ok()?;

                            self.next()
                        },
                        // Array Descriptor (64-bit)
                        TFormType::Q { ty, t_byte_size, .. } => {
                            let n_elems = reader.read_u64::<BigEndian>().ok()?;
                            let byte_offset = reader.read_u64::<BigEndian>().ok()?;

                            self.byte_offset += Q::BYTES_SIZE;

                            let colname = self.ctx.ttypes[col_idx].unwrap_or(String::from(""));
                            self.reader = self.reader.jump_to_heap(
                                &self.ctx,
                                &colname,
                                byte_offset as u64,
                                self.byte_offset as u64,
                                *ty,
                                n_elems * (*t_byte_size)
                            ).ok()?;

                            self.next()
                        }
                    }
                }
            },
            DataReader::HEAP { reader, num_bytes_to_read, ty, .. } => {
                let value = reader.read_u32::<BigEndian>().ok()?;
                

                match ty {
                    // short
                    'I' => {
                        *num_bytes_to_read -= (I::BYTES_SIZE as u64);
                        Some(DataValue::Short { value: value as i16, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                    },
                    'J' =>  {
                        *num_bytes_to_read -= (J::BYTES_SIZE as u64);
                        Some(DataValue::Integer { value: value as i32, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                    },
                    'K' =>  {
                        *num_bytes_to_read -= (K::BYTES_SIZE as u64);
                        Some(DataValue::Long { value: value as i64, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                    },
                    'E' =>  {
                        *num_bytes_to_read -= (E::BYTES_SIZE as u64);
                        Some(DataValue::Float { value: value as f32, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                    },
                    'D' =>  {
                        *num_bytes_to_read -= (D::BYTES_SIZE as u64);
                        Some(DataValue::Double { value: value as f64, column: ColumnId::Index(col_idx), idx: 0 }) // Determine the count idx inside the field
                    },
                    _ => unreachable!() // TODO
                }


            }
        }
    }
}

use async_trait::async_trait;
use futures::AsyncReadExt;
use std::marker::Unpin;

#[async_trait(?Send)]
impl<'a, R> AsyncDataBufRead<'a, BinTable> for futures::io::BufReader<R>
where
    R: AsyncReadExt + Debug + Unpin + 'a,
{
    type Data = St<'a, Self, u8>;

    fn prepare_data_reading(
        _ctx: &BinTable,
        num_remaining_bytes_in_cur_hdu: &'a mut usize,
        reader: &'a mut Self,
    ) -> Self::Data {
        St::new(reader, num_remaining_bytes_in_cur_hdu)
    }
}