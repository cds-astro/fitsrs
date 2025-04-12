//! This code is a port in Rust of CFITSIO's ricecomp.c
//!
//! The original code can be found here: <https://github.com/HEASARC/cfitsio/blob/develop/ricecomp.c#L1>
//! The port tends to provide a rust idiomatic RICE decoder reader that can operate on top of
//! another reader coming for example from a web stream, a file, a memory-map.

/*
 * nonzero_count is lookup table giving number of bits in 8-bit values not including
 * leading zeros used in fits_rdecomp, fits_rdecomp_short and fits_rdecomp_byte
 */
const NONZERO_COUNT: [i32; 256] = [
    0, 1, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5,
    6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6, 6,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
    8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8,
];

#[derive(Debug)]
enum RICEState {
    /// First bytes are not encoded
    Start,
    /// FS entry value
    FS {
        b: u32,
        i: i32,
        lastpix: i32,
        nbits: i32,
    },
    /// Low entropy case, values are equal
    LowEntropyDecoding {
        b: u32,
        i: i32,
        imax: i32,
        lastpix: i32,
        nbits: i32,
    },
    /// High entropy case, difference of adjacent pixels
    HighEntropyDecoding {
        b: u32,
        i: i32,
        imax: i32,
        nbits: i32,
        lastpix: i32,
    },
    /// RICE decoding
    RICEDecoding {
        b: u32,
        fs: i32,
        i: i32,
        imax: i32,
        lastpix: i32,
        nbits: i32,
    },
}

use std::marker::PhantomData;
#[derive(Debug)]
pub(crate) struct RICEDecoder<R, T> {
    /// The reader to decorate
    pub reader: R,
    /// Internal variable to know in which state the reader is
    state: RICEState,
    /// Coding block size
    nblock: i32,
    /// Number of output pixels/values
    nx: i32,
    /// Type of the output value after decoding
    _t: PhantomData<T>,
}

impl<R, T> RICEDecoder<R, T> {
    /// Init a RICE decoder decorator on a reader
    /// This does nothing
    ///
    /// # Params
    ///
    /// * `reader` - The reader to decode
    /// * `nblock` - coding block size, usually 32 is given
    /// * `nx` - number of output pixels/values
    pub(crate) fn new(reader: R, nblock: i32, nx: i32) -> Self {
        Self {
            reader,
            state: RICEState::Start,
            nblock,
            nx,
            _t: PhantomData,
        }
    }
}

use std::io::Error;
/// A trait to define constant for possible output types from a
/// RICE decoder/encoder
trait RICE: Sized + Into<i32> {
    const FSBITS: i32;
    const FSMAX: i32;
    const BBITS: i32 = 1 << Self::FSBITS;

    /// Just a wrapping around std::mem::size_of
    fn size_of() -> usize {
        std::mem::size_of::<Self>()
    }

    /// Utilitary method for reading the first T elem that is not encoded
    fn from_be_bytes<R: Read>(reader: &mut R) -> Result<Self, Error>;
}

impl RICE for u8 {
    const FSBITS: i32 = 3;
    const FSMAX: i32 = 6;

    fn from_be_bytes<R: Read>(reader: &mut R) -> Result<Self, Error> {
        reader.read_u8()
    }
}

impl RICE for i16 {
    const FSBITS: i32 = 4;
    const FSMAX: i32 = 14;

    fn from_be_bytes<R: Read>(reader: &mut R) -> Result<Self, Error> {
        reader.read_i16::<BigEndian>()
    }
}

impl RICE for i32 {
    const FSBITS: i32 = 5;
    const FSMAX: i32 = 25;

    fn from_be_bytes<R: Read>(reader: &mut R) -> Result<Self, Error> {
        reader.read_i32::<BigEndian>()
    }
}

use std::io::Read;

use byteorder::BigEndian;
use byteorder::ReadBytesExt;
impl<R, T> Read for RICEDecoder<R, T>
where
    R: Read,
    T: RICE,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // a counter of the num bytes read through that read call
        let mut j = 0;
        loop {
            match self.state {
                RICEState::Start => {
                    let lastpix = T::from_be_bytes(&mut self.reader)?.into();

                    /* bit buffer			*/
                    let b = self.reader.read_u8()? as u32;
                    /* number of bits remaining in b	*/
                    let nbits = 8;
                    let i = 0;
                    self.state = RICEState::FS {
                        nbits,
                        b,
                        i,
                        lastpix,
                    };
                }
                RICEState::FS {
                    mut nbits,
                    mut b,
                    i,
                    lastpix,
                } => {
                    /* get the FS value from first fsbits */
                    nbits -= T::FSBITS;
                    while nbits < 0 {
                        b = (b << 8) | (self.reader.read_u8()? as u32);
                        nbits += 8;
                    }
                    let fs = (b >> nbits) as i32 - 1;

                    b &= (1 << nbits) - 1;
                    /* loop over the next block */
                    let imax = (i + self.nblock).min(self.nx);

                    self.state = if fs < 0 {
                        RICEState::LowEntropyDecoding {
                            i,
                            lastpix,
                            imax,
                            b,
                            nbits,
                        }
                    } else if fs == T::FSMAX {
                        RICEState::HighEntropyDecoding {
                            i,
                            b,
                            nbits,
                            lastpix,
                            imax,
                        }
                    } else {
                        RICEState::RICEDecoding {
                            i,
                            nbits,
                            b,
                            fs,
                            lastpix,
                            imax,
                        }
                    }
                }
                RICEState::LowEntropyDecoding {
                    b,
                    nbits,
                    mut i,
                    lastpix,
                    imax,
                } => {
                    /* low-entropy case, all zero differences */
                    while j < buf.len() && i < imax {
                        buf[j..(j + T::size_of())]
                            .copy_from_slice(&lastpix.to_ne_bytes()[..T::size_of()]);

                        j += T::size_of();
                        i += 1;
                    }

                    if i == imax {
                        // We processed all the block, we need to recompute a FS
                        self.state = RICEState::FS {
                            nbits,
                            b,
                            i,
                            lastpix,
                        }
                    }

                    // The buf has been filled
                    if j == buf.len() {
                        return Ok(j);
                    }
                }
                RICEState::HighEntropyDecoding {
                    nbits,
                    mut b,
                    mut i,
                    mut lastpix,
                    imax,
                } => {
                    while j < buf.len() && i < imax {
                        let mut k = T::BBITS - nbits;
                        let mut diff = ((b as u64) << k) as u32;

                        k -= 8;
                        while k >= 0 {
                            b = self.reader.read_u8()? as u32;
                            diff |= b << k;

                            k -= 8;
                        }
                        if nbits > 0 {
                            b = self.reader.read_u8()? as u32;
                            diff |= b >> (-k);
                            b &= (1 << nbits) - 1;
                        } else {
                            b = 0;
                        }
                        /*
                         * undo mapping and differencing
                         * Note that some of these operations will overflow the
                         * unsigned int arithmetic -- that's OK, it all works
                         * out to give the right answers in the output file.
                         */
                        if (diff & 1) == 0 {
                            diff >>= 1;
                        } else {
                            diff = !(diff >> 1);
                        }

                        let curpix = (diff as i32) + lastpix;
                        buf[j..(j + T::size_of())]
                            .copy_from_slice(&curpix.to_ne_bytes()[..T::size_of()]);
                        lastpix = curpix;

                        i += 1;
                        j += T::size_of();
                    }

                    if i == imax {
                        // We processed all the block, we need to recompute a FS
                        self.state = RICEState::FS {
                            nbits,
                            b,
                            i,
                            lastpix,
                        }
                    }

                    // The buf has been filled
                    if j == buf.len() {
                        return Ok(j);
                    }
                }
                RICEState::RICEDecoding {
                    mut nbits,
                    mut b,
                    mut i,
                    mut lastpix,
                    fs,
                    imax,
                } => {
                    while j < buf.len() && i < imax {
                        /* count number of leading zeros */
                        while b == 0 {
                            nbits += 8;
                            b = self.reader.read_u8()? as u32;
                        }
                        let nzero = nbits - NONZERO_COUNT[b as usize];
                        nbits -= nzero + 1;
                        /* flip the leading one-bit */
                        b ^= 1 << nbits;
                        /* get the FS trailing bits */
                        nbits -= fs;
                        while nbits < 0 {
                            b = (b << 8) | (self.reader.read_u8()? as u32);
                            nbits += 8;
                        }
                        let mut diff = ((nzero as u32) << fs) | (b >> nbits);
                        b &= (1 << nbits) - 1;

                        /* undo mapping and differencing */
                        if (diff & 1) == 0 {
                            diff >>= 1;
                        } else {
                            diff = !(diff >> 1);
                        }
                        let curpix = (diff as i32) + lastpix;
                        buf[j..(j + T::size_of())]
                            .copy_from_slice(&curpix.to_ne_bytes()[..T::size_of()]);
                        lastpix = curpix;

                        i += 1;
                        j += T::size_of();
                    }

                    if i == imax {
                        // We processed all the block, we need to recompute a FS
                        self.state = RICEState::FS {
                            nbits,
                            b,
                            i,
                            lastpix,
                        }
                    }

                    // The buf has been filled
                    if j == buf.len() {
                        return Ok(j);
                    }
                }
            }
        }
    }
}
