Fits reader written in pure Rust
--------------------------------

[![](https://img.shields.io/crates/v/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![](https://img.shields.io/crates/d/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![API Documentation on docs.rs](https://docs.rs/fitsrs/badge.svg)](https://docs.rs/fitsrs/)
![testing CI](https://github.com/cds-astro/fitsrs/actions/workflows/rust.yml/badge.svg)

This crate is under development, it was initiated for reading fits HiPS tile, i.e. generated from hipsgen.

This fits parser only supports image data (not tables), and does not know anything about WCS parsing.
For WCS parsing, see [wcsrs](https://github.com/cds-astro/wcs-rs).
This parser is able to parse extension HDUs. Ascii tables and binary tables are still not properly parsed, only the list bytes of their data block can be retrieved but no interpretation/parsing is done on it.

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
* [X] Support big fits file parsing that may not fit in memory (iterator usage)
* [X] Async reading (experimental and not tested)
* [ ] Keep CARD comment
* [ ] Support compressed fits files (https://fits.gsfc.nasa.gov/registry/tilecompression.html)
* [ ] Support data table (each column can have a specific types)
* [X] Support of multiple HDU, fits extensions (in progress, only the header is parsed)
* [ ] WCS parsing, see [wcsrs](https://github.com/cds-astro/wcs-rs)

Example
----------

For files that can fit in memory

```rust
use std::fs::File;
use std::io::Cursor;
use std::io::Read;
use fitsrs::{Fits, HDU};
use fitsrs::{hdu::header::Xtension, hdu::data::Data};

let mut f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();

let mut buf = Vec::new();
f.read_to_end(&mut buf).unwrap();
let reader = Cursor::new(&buf[..]);

let mut hdu_list = Fits::from_reader(reader);

// Access the HDU extensions
while let Some(Ok(hdu)) = hdu_list.next() {
    match hdu {
        HDU::Primary(_) => (),
        HDU::XImage(hdu) => {
            let xtension = hdu.get_header().get_xtension();

            let naxis1 = *xtension.get_naxisn(1).unwrap() as usize;
            let naxis2 = *xtension.get_naxisn(2).unwrap() as usize;

            let num_pixels = naxis2 * naxis1;

            match hdu.get_data(&mut hdu_list) {
                Data::U8(mem) => assert_eq!(num_pixels, mem.len()),
                Data::I16(mem) => assert_eq!(num_pixels, mem.len()),
                Data::I32(mem) => assert_eq!(num_pixels, mem.len()),
                Data::I64(mem) => assert_eq!(num_pixels, mem.len()),
                Data::F32(mem) => assert_eq!(num_pixels, mem.len()),
                Data::F64(mem) => assert_eq!(num_pixels, mem.len()),
            }
        },
        HDU::XBinaryTable(hdu) => {
            let num_bytes = hdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            match hdu.get_data(&mut hdu_list) {
                Data::U8(mem) => assert_eq!(num_bytes as usize, mem.len()),
                _ => unreachable!()
            }
        },
        HDU::XASCIITable(hdu) => {
            let num_bytes = hdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            match hdu.get_data(&mut hdu_list) {
                Data::U8(mem) => assert_eq!(num_bytes as usize, mem.len()),
                _ => unreachable!()
            }
        },
    }
}
```

For BufReader

```rust
use std::fs::File;
use std::io::Cursor;
use fitsrs::hdu::HDU;
use fitsrs::hdu::data::DataIter;
use fitsrs::fits::Fits;
use fitsrs::hdu::header::Xtension;

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

            match hdu.get_data(&mut hdu_list) {
                DataIter::U8(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataIter::I16(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataIter::I32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataIter::I64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataIter::F32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataIter::F64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
            }
        },
        HDU::XBinaryTable(hdu) => {
            let num_bytes = hdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let it_bytes = hdu.get_data(&mut hdu_list);
            let data = it_bytes.collect::<Vec<_>>();
            assert_eq!(num_bytes as usize, data.len());
        },
        HDU::XASCIITable(hdu) => {
            let num_bytes = hdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let it_bytes = hdu.get_data(&mut hdu_list);
            let data = it_bytes.collect::<Vec<_>>();
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

                match hdu.get_data(&mut hdu_list) {
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

                let it_bytes = hdu.get_data(&mut hdu_list);
                let data = it_bytes.collect::<Vec<_>>().await;
                assert_eq!(num_bytes as usize, data.len());
            },
            AsyncHDU::XASCIITable(hdu) => {
                let num_bytes = xhdu.get_header()
                    .get_xtension()
                    .get_num_bytes_data_block();

                let it_bytes = xhdu.get_data(&mut hdu_list);
                let data = it_bytes.collect::<Vec<_>>().await;
                assert_eq!(num_bytes as usize, data.len());
            },
        }
    }
}
```