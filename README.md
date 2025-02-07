FITS file reader written in pure Rust
-------------------------------------

[![](https://img.shields.io/crates/v/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![](https://img.shields.io/crates/d/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![API Documentation on docs.rs](https://docs.rs/fitsrs/badge.svg)](https://docs.rs/fitsrs/)
![testing CI](https://github.com/cds-astro/fitsrs/actions/workflows/rust.yml/badge.svg)

This crate is under development, it was initiated for reading FITS images mapped onto HEALPix cell in the sky (See the [HiPS IVOA](https://www.ivoa.net/documents/HiPS/) standard) for using inside the [Aladin Lite](https://github.com/cds-astro/aladin-lite) web sky atlas.

Currently, fitsrs supports reading multiple HDU and is mainly dedicated to image extension reading.
For interpreting WCS keywords, see [wcs-rs](https://github.com/cds-astro/wcs-rs).
A very new support of binary table extension has been added. This has been done mainly for supporting the tiled image convention for storing compressed images in binary tables. This stores tile images inside variable length arrays of a binary table.
The ASCII table extension parsing has not been implemented but it is possible to get an iterator over the data bytes as well as its mandatory cards from the header.

Contributing
------------

> [!WARNING]
> Running the test involves test files you can download [here](https://alasky.cds.unistra.fr/Aladin-Lite-test-files/fits-rs-test-files.tar). This tar is 2.2GB.

Once the tar file has been downloaded, put it into the root on your cloned repo and extract it:

```bash
tar -xvf fits-rs-test-files.tar
```

Once the files have been extracted you can run the tests locally:

```bash
cargo test --release
```

To Do list
----------

* [X] Support single typed data block (i.e. image type data)
* [X] Single HDU parsing, header and data units 
* [X] Support FITS files that may not fit in memory (iterator, possibility to seek directly to a specific pixel index/row)
* [X] Async reading (requires to read the whole data. Seeking is not possible)
* [X] Parsing of comments, history, continued, cards.
* [X] Keep all the cards in the original order
* [X] Basic support of Bintable
* [X] Tiled image convention for storing compressed images in FITS binary tables
    - [X] Compression supported, GZIP, GZIP2 and RICE on u8, i16, i32 and f32.
    - [ ] H_compress and PLI0 are not supported
    - [X] Dithering techniques for floating point images. Not well tested (test samples are welcome)
    - [ ] `NULL_PIXEL_MASK` column and `ZMASKCMP` keyword is not supported
* [ ] FITS writer/serializer
* [ ] ASCII table extension parsing
* [ ] Tile-compressed in binary table files (https://fits.gsfc.nasa.gov/registry/tilecompression.html). Only RICE and GZIP supported
* [X] Support of multiple HDU. Image and binary tables extension support. Provide an idiomatic Rust iterator over the list of HDU.
* [X] WCS parsing, see [wcs-rs](https://github.com/cds-astro/wcs-rs)

> [!WARNING]
> Features not done are not planned to be done. A work for supporting them can be done only if people have use cases for those i.e. it is only at user's requests. The FITS standard and its conventions are massive and it is a huge work to support all the cases and sub cases.


License
-------

fitsrs has the double the MIT/Apache-2.0 license.

It uses code adapted from the famous [CFITSIO](https://github.com/HEASARC/cfitsio/blob/main/licenses/License.txt) library.Especially the RICE compression/decompression source code has been ported from the original cfitsio [code](https://github.com/HEASARC/cfitsio/blob/main/ricecomp.c) to Rust.

Example
----------

```rust
use std::fs::File;
use std::io::Cursor;
use fitsrs::{Fits, ImageData, HDU, hdu::header::Xtension};

use std::io::{BufReader, Read};

let f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
let reader = BufReader::new(f);

let mut hdu_list = Fits::from_reader(reader);

while let Some(Ok(hdu)) = hdu_list.next() {
    match hdu {
        // skip the primary HDU
        HDU::Primary(_) => (),
        HDU::XImage(hdu) => {
            let xtension = hdu.get_header().get_xtension();

            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();

            let num_pixels = (naxis2 * naxis1) as usize;

            match hdu_list.get_data(hdu) {
                ImageData::U8(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                ImageData::I16(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                ImageData::I32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                ImageData::I64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                ImageData::F32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                ImageData::F64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
            }
        },
        HDU::XBinaryTable(hdu) => {
            let num_rows = hdu.get_header()
                .get_xtension()
                .get_num_rows();

            let rows = hdu_list.get_data(hdu)
                .row_iter()
                .collect::<Vec<_>>();

            assert_eq!(num_rows, rows.len());
        },
        HDU::XASCIITable(hdu) => {
            let num_bytes = hdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let data = hdu_list.get_data(hdu)
                .collect::<Vec<_>>();

            assert_eq!(num_bytes as usize, data.len());
        },
    }
}
```

For async input readers:

```rust
#[tokio::test]
async fn parse_fits_async() {
    use std::fs::File;
    use std::io::Cursor;
    use fitsrs::hdu::AsyncHDU;
    use fitsrs::hdu::data::stream::Stream;
    use fitsrs::async_fits::AsyncFits;
    use fitsrs::hdu::header::extension::Xtension;

    use std::io::{BufReader, Read};

    // reader needs to implement futures::io::AsyncRead
    let f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
    let reader = BufReader::new(f);

    let mut hdu_list = AsyncFits::from_reader(reader);

    while let Some(Ok(mut hdu)) = hdu_list.next().await {
        match hdu {
            AsyncHDU::Primary(_) => (),
            AsyncHDU::Image(hdu) => {
                let xtension = hdu.get_header().get_xtension();

                let naxis1 = *xtension.get_naxisn(1).unwrap() as usize;
                let naxis2 = *xtension.get_naxisn(2).unwrap() as usize;

                let num_pixels = naxis2 * naxis1;

                match hdu_list.get_data(hdu) {
                    Stream::U8(st) => {
                        let data = st.collect::<Vec<_>>().await;
                        assert_eq!(num_pixels, data.len())
                    },
                    Stream::I16(st) => {
                        let data = st.collect::<Vec<_>>().await;
                        assert_eq!(num_pixels, data.len())
                    },
                    Stream::I32(st) => {
                        let data = st.collect::<Vec<_>>().await;
                        assert_eq!(num_pixels, data.len())
                    },
                    Stream::I64(st) => {
                        let data = st.collect::<Vec<_>>().await;
                        assert_eq!(num_pixels, data.len())
                    },
                    Stream::F32(st) => {
                        let data = st.collect::<Vec<_>>().await;
                        assert_eq!(num_pixels, data.len())
                    },
                    Stream::F64(st) => {
                        let data = st.collect::<Vec<_>>().await;
                        assert_eq!(num_pixels, data.len())
                    },
                }
            },
            AsyncHDU::XBinaryTable(hdu) => {
                let num_bytes = hdu.get_header()
                    .get_xtension()
                    .get_num_bytes_data_block();

                let it_bytes = hdu_list.get_data(hdu);
                let data = it_bytes.collect::<Vec<_>>().await;
                assert_eq!(num_bytes as usize, data.len());
            },
            AsyncHDU::XASCIITable(hdu) => {
                let num_bytes = xhdu.get_header()
                    .get_xtension()
                    .get_num_bytes_data_block();

                let it_bytes = hdu_list.get_data(hdu);
                let data = it_bytes.collect::<Vec<_>>().await;
                assert_eq!(num_bytes as usize, data.len());
            },
        }
    }
}
```