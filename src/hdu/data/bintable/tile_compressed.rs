use flate2::read::GzDecoder;

use super::data::TableData;
use super::rice::RICEDecoder;
use crate::card::Value;
use crate::error::Error;
use crate::hdu::header::{Header, Bitpix};
use crate::hdu::header::extension::bintable::{TileCompressedImage, BinTable, ZCmpType};

use super::row::TableRowData;

struct TileCompressedData<R> {
    /// An iterator over the row of a binary table
    row_it: TableRowData<R>,
    /// A buffer for storing uncompressed data from GZIP1, GZIP2 or RICE
    buf: Vec<u8>,
    
    // Mandatory field
    data_compressed_idx: usize,
    // Optional floating point columns
    z_scale_idx: Option<usize>,
    z_zero_idx: Option<usize>,
    // Optional column or keyword
    z_blank: Option<ZBLANK>,
    /// The type of compression algorithm
    z_cmp_type: ZCmpType,
    /// Bitpix of the uncompressed values
    z_bitpix: Bitpix,
    /// Tile size
    z_tile: Box<[usize]>,
    /// Total image size
    z_naxis: Box<[usize]>,

    /// Current tile pointer
    tile_compressed: TileCompressed,
    /// A boolean telling when a tile has been completed so that we parse the next row
    remaining_pixels: u64,
}

struct TileCompressed {
    /// Current value of ZSCALE field (floating point case)
    z_scale: f64,
    /// Current value of ZZERO field (floating point case)
    z_zero: f64,
    /// Current value of ZBLANK field (float and integer case)
    z_blank: Option<f64>,
}

enum ZBLANK {
    ColumnIdx(usize),
    Value(f64)
}

use std::fmt::Debug;
use std::io::{Read, Seek, SeekFrom};

impl<R> TileCompressedData<R> {
    /// Compute the size of the tile from its row position inside the compressed data column
    /// FIXME: optimize this computation by using memoization
    fn tile_size_from_row_idx(&self, n: usize) -> Box<[usize]> {
        let d = self.z_tile.len();

        if d == 0 {
            // There must be at least one dimension
            unreachable!();
        } else {
            let mut u = vec![0_usize; self.z_tile.len()];

            let s = self.z_naxis.iter().zip(self.z_tile.iter()).map(|(naxisi, tilei)| {
                naxisi.div_ceil(*tilei)
            }).collect::<Vec<_>>();

            // Compute the position inside the first dimension
            u[0] = n % s[0];

            for i in 1..=(d - 1) {
                u[i] = n - u[0] - (1..=(i - 1)).map(|k| {
                    let mut prod_sk = 1;
                    for l in 0..=(k - 1) {
                        prod_sk *= s[l];
                    }

                    u[k] * prod_sk
                }).sum::<usize>();

                let mut prod_si = 1;
                for k in 0..=(i - 1) {
                    prod_si *= s[k];
                }

                u[i] = (u[i] / prod_si) % s[i];
            }

            u.iter().zip(self.z_naxis.iter().zip(self.z_tile.iter())).map(|(&u_i, (&naxis, &tilez))| {
                tilez.min(naxis - u_i*tilez)
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
        }
    }

    fn get_reader(&mut self) -> &mut R {
        self.row_it.get_reader()
    }
}

impl<R> TileCompressedData<R>
where
    R: Debug + Read + Seek
{
    fn new(header: &Header<BinTable>, mut data: TableData<R>, config: &TileCompressedImage) -> Result<Self, Error> {
        // This buffer is only used if tile compressed image in the gzip compression is to be found
        let mut buf = vec![];

        let TileCompressedImage {
            z_tilen,
            z_naxisn,
            z_bitpix,
            z_cmp_type,
            ..
        } = config;
        // Disable reading the heap, we will handle that manually after reading the column
        data.read_the_heap(false);
        let row_it = data.row_iter();

        let ctx = header.get_xtension();

        let z_scale_idx = ctx.find_field_by_ttype("ZSCALE");
        let z_zero_idx = ctx.find_field_by_ttype("ZZERO");

        // Allocation of a buffer at init of the iterator that is the size of the biggest tiles we can found
        // on the tile compressed image. This is simply given by the ZTILEi keyword.
        // Some tiles found on the border of the image can be smaller
        let n_elems_max = z_tilen.iter().fold(1, |mut tile_size, z_tilei| {
            tile_size *= *z_tilei;
            tile_size
        });

        // FIXME
        // A little precision. The gzdecoder from flate2 seems to unzip data in a stream of u32 i.e. even if the type of data
        // to uncompress is byte or short, the result will be cast in a u32. I thus allocate for gzip compression, a buffer of
        // n_elems_max * size_in_bytes of u32.
        // It seems to be the same for RICE, so the latter rice decompression code is called on i32
        let num_bytes_max_tile = n_elems_max * std::mem::size_of::<u32>();

        buf.resize(num_bytes_max_tile, 0);
        
        let z_blank = header.get_xtension()
            // Check for a field named ZBLANK
            .find_field_by_ttype("ZBLANK")
            // If no ZBLANK colum has been found then check the header keywords (ZBLANK for float, BLANK for integer)
            .map_or_else(
                || {
                    // integers
                    if (*z_bitpix as i32) < 0 {
                        if let Some(Value::Float { value, .. }) = header.get("ZBLANK") {
                            Some(ZBLANK::Value(*value))
                        } else {
                            None
                        }
                    } else {
                        if let Some(Value::Integer { value, .. }) = header.get("BLANK") {
                            Some(ZBLANK::Value(*value as f64))
                        } else {
                            None
                        }
                    }
                },
                |field_idx| {
                    Some(ZBLANK::ColumnIdx(field_idx))
                }
            );

        let data_compressed_idx = ctx
            // Find for a DATA_COMPRESSED named field
            .find_field_by_ttype("DATA_COMPRESSED")
            // Find for a GZIP_DATA_COMPRESSED named field
            .or(ctx.find_field_by_ttype("GZIP_DATA_COMPRESSED"))
            // As we have a config object we know there is a DATA_COMPRESSED field
            .ok_or(Error::StaticError("DATA_COMPRESSED or GZIP_COMPRESSED_DATA fields are not found"))?;

        let tile_compressed = TileCompressed { z_scale: 1.0, z_zero: 0.0, z_blank: None };

        Ok(Self {
            buf,
            row_it,
            tile_compressed,
            data_compressed_idx,
            z_scale_idx,
            z_zero_idx,
            z_blank,
            z_naxis: z_naxisn.clone(),
            z_tile: z_tilen.clone(),
            remaining_pixels: 0,
            z_cmp_type: *z_cmp_type,
            z_bitpix: *z_bitpix,
        })
    }
}

use super::DataValue;
impl<R> Iterator for TileCompressedData<R>
where
    R: Read + Seek + Debug,
{
    // Return a vec of fields because to take into account the repeat count value for that field
    type Item = DataValue;

    fn next(&mut self) -> Option<Self::Item> {
        // We first retrieve the whole row from the row data iterator
        if self.remaining_pixels == 0 {
            let row_data = self.row_it.next()?;

            let (n_elems, byte_offset) = match row_data[self.data_compressed_idx] {
                DataValue::VariableLengthArray32 { num_elems, offset_byte } => (num_elems as u64, offset_byte as u64),
                DataValue::VariableLengthArray64 { num_elems, offset_byte } => (num_elems as u64, offset_byte as u64),
                _ => unreachable!()
            };

            let z_scale = if let Some(idx) = self.z_scale_idx {
                match row_data[idx] {
                    DataValue::Float { value, .. } => value as f64,
                    DataValue::Double { value, .. } => value,
                    _ => unreachable!()
                }
            } else {
                1.0
            };

            let z_zero = if let Some(idx) = self.z_zero_idx {
                match row_data[idx] {
                    DataValue::Float { value, .. } => value as f64,
                    DataValue::Double { value, .. } => value,
                    _ => unreachable!()
                }
            } else {
                0.0
            };

            let z_blank = match self.z_blank {
                Some(ZBLANK::ColumnIdx(idx)) => {
                    match row_data[idx] {
                        DataValue::Float { value, .. } => Some(value as f64),
                        DataValue::Double { value, .. } => Some(value),
                        _ => unreachable!()
                    }
                }
                Some(ZBLANK::Value(value)) => Some(value),
                _ => None
            };

            self.tile_compressed = TileCompressed {
                z_scale,
                z_blank,
                z_zero
            };

            // We must manage manually the heap because we disabled the automatic heap seeking
            // Move to the HEAP
            let ctx = self.row_it.get_ctx();
            let row_idx = self.row_it.get_row_idx();

            let main_data_table_offset = row_idx * (ctx.naxis1 as usize);
            let off =
                // go back to the beginning of the main table data block
                - (main_data_table_offset as i64)
                // from the beginning of the main table go to the beginning of the heap
                + ctx.theap as i64
                // from the beginning of the heap go to the start of the array
                + byte_offset as i64;
 
            // Go to the HEAP
            self.jump_to_location(|s| {
                s.remaining_pixels = s.tile_size_from_row_idx(row_idx).iter().fold(1, |mut n, &tile| {
                    n *= tile;
                    n
                }) as u64;

                let TileCompressedData { buf, row_it, .. } = s;
                let reader = row_it.get_reader();

                match s.z_cmp_type {
                    // For GZIP2, the byte shuffling is done in the next method
                    ZCmpType::Gzip1 | ZCmpType::Gzip2 => {
                        let mut gz = GzDecoder::new(reader);
                        gz.read_exact(&mut buf[..])?;
                    }
                    // FIXME support bytepix
                    ZCmpType::Rice { blocksize, .. } => {
                        let mut rice = RICEDecoder::<_, i32>::new(reader, blocksize as i32, n_elems as i32);
                        rice.read_exact(&mut buf[..])?;
                    }
                    // Other compression not supported, when parsing the bintable extension keywords
                    // we ensured that z_image is `None` for other compressions than GZIP or RICE
                    _ => unreachable!()
                }

                Ok(())
            }, SeekFrom::Current(off)).ok()?;


        } else {

        }

        // 



        None
    }
}

impl<R> TileCompressedData<R>
where
    R: Seek
{
    /// Jump to a specific location of the reader, perform an operation and jumps back to the original position
    pub(crate) fn jump_to_location<F>(&mut self, f: F, pos: SeekFrom) -> Result<(), Error>
    where
        F: FnOnce(&mut Self) -> Result<(), Error>
    {
        let old_pos = SeekFrom::Start(self.get_reader().stream_position()?);

        self.get_reader().seek(pos)?;
        f(self)?;
        let _ = self.get_reader().seek(old_pos)?;

        Ok(())
    }
}




