Fits reader written in pure Rust using [nom](https://github.com/Geal/nom)
-------------------------------------------------------------------------

[![](https://img.shields.io/crates/v/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![](https://img.shields.io/crates/d/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![API Documentation on docs.rs](https://docs.rs/fitsrs/badge.svg)](https://docs.rs/fitsrs/)
![testing CI](https://github.com/cds-astro/fitsrs/actions/workflows/rust.yml/badge.svg)

This crate is under development, it was initiated for reading fits HiPS tile, i.e. generated from hipsgen.

This fits parser only supports image data (not tables), and does not know anything about WCS parsing.
For WCS parsing, see [wcsrs](https://github.com/cds-astro/wcs-rs).
This parser is able to parse extension HDUs. Ascii tables and binary tables are still not properly parsed, only the list bytes of their data block can be retrieved but no interpretation/parsing is done on it.

Cloning the Repositiory
-----------------------
Make sure that you have `git-lfs` installed. If you an error along the following lines:
> Downloading samples/fits.gsfc.nasa.gov/Astro_UIT.fits (865 KB)
Error downloading object: samples/fits.gsfc.nasa.gov/Astro_UIT.fits (3110c73): Smudge error: Error downloading samples/fits.gsfc.nasa.gov/Astro_UIT.fits (3110c73eebbdd479b9a51cb275f18650f17d22b818713562b6e27d873e128530): batch response: This repository is over its data quota. Account responsible for LFS bandwidth should purchase more data packs to restore access.

(cf. Issue #10)

... try the following:

```
GIT_LFS_SKIP_SMUDGE=1 git clone git@github.com:cds-astro/fitsrs.git
```
This clones the repository without the large binary files and still allows you to build locally.

To Do list
----------

* [X] Support single typed data block (i.e. image type data)
* [X] Single HDU parsing, header and data units 
* [X] Support big fits file parsing that may not fit in memory (iterator usage)
* [X] Async reading (experimental and not tested)
* [ ] Keep CARD comment
* [ ] Support data table (each column can have a specific types)
* [X] Support of multiple HDU, fits extensions (in progress, only the header is parsed)
* [ ] WCS parsing, see [wcsrs](https://github.com/cds-astro/wcs-rs)

Example
----------

For files that can fit in memory
```rust
use fitsrs::{
    fits::Fits,
    hdu::{
        data::InMemData,
        extension::XtensionHDU
    }
};

use std::fs::File;
use std::io::Cursor;

let mut f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();

let mut buf = Vec::new();
f.read_to_end(&mut buf).unwrap();
let mut reader = Cursor::new(&buf[..]);

let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

// Access the HDU extensions
let mut hdu_ext = hdu.next();

while let Ok(Some(hdu)) = hdu_ext {
    match &hdu {
        XtensionHDU::Image(xhdu) => {
            let xtension = xhdu.get_header().get_xtension();

            let naxis1 = *xtension.get_naxisn(1).unwrap() as usize;
            let naxis2 = *xtension.get_naxisn(2).unwrap() as usize;

            let num_pixels = naxis2 * naxis1;

            match xhdu.get_data() {
                InMemData::U8(mem) => assert_eq!(num_pixels, mem.len()),
                InMemData::I16(mem) => assert_eq!(num_pixels, mem.len()),
                InMemData::I32(mem) => assert_eq!(num_pixels, mem.len()),
                InMemData::I64(mem) => assert_eq!(num_pixels, mem.len()),
                InMemData::F32(mem) => assert_eq!(num_pixels, mem.len()),
                InMemData::F64(mem) => assert_eq!(num_pixels, mem.len()),
            }
        },
        XtensionHDU::BinTable(xhdu) => {
            let num_bytes = xhdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            match xhdu.get_data() {
                InMemData::U8(mem) => assert_eq!(num_bytes as usize, mem.len()),
                _ => unreachable!()
            }
        },
        XtensionHDU::AsciiTable(xhdu) => {
            let num_bytes = xhdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            match xhdu.get_data() {
                InMemData::U8(mem) => assert_eq!(num_bytes as usize, mem.len()),
                _ => unreachable!()
            }
        },
    }

    hdu_ext = hdu.next();
}
```

For files that may not be contained into the memory
```rust
use fitsrs::{
    fits::Fits,
    hdu::{
        data::iter,
        extension::XtensionHDU
    }
};

use std::fs::File;
use std::io::{BufReader, Read};

let f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
let mut reader = BufReader::new(f);

let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

let mut hdu_ext = hdu.next();

while let Ok(Some(mut xhdu)) = hdu_ext {
    match &mut xhdu {
        XtensionHDU::Image(xhdu) => {
            let xtension = xhdu.get_header().get_xtension();

            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();

            let num_pixels = (naxis2 * naxis1) as usize;

            match xhdu.get_data_mut() {
                iter::Data::U8(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                iter::Data::I16(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                iter::Data::I32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                iter::Data::I64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                iter::Data::F32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                iter::Data::F64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
            }
        },
        XtensionHDU::BinTable(xhdu) => {
            let num_bytes = xhdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let it_bytes = xhdu.get_data_mut();
            let data = it_bytes.collect::<Vec<_>>();
            assert_eq!(num_bytes as usize, data.len());
        },
        XtensionHDU::AsciiTable(xhdu) => {
            let num_bytes = xhdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let it_bytes = xhdu.get_data_mut();
            let data = it_bytes.collect::<Vec<_>>();
            assert_eq!(num_bytes as usize, data.len());
        },
    }

    hdu_ext = xhdu.next();
}
```

For async input readers:

```rust
use fitsrs::{
    fits::AsyncFits,
    hdu::{
        data::stream,
        extension::AsyncXtensionHDU
    }
};

// reader needs to implement futures::io::AsyncRead
let AsyncFits { hdu } = AsyncFits::from_reader(&mut reader).await.unwrap();

let mut hdu_ext = hdu.next().await;

while let Ok(Some(mut xhdu)) = hdu_ext {
    match &mut xhdu {
        AsyncXtensionHDU::Image(xhdu) => {
            let xtension = xhdu.get_header().get_xtension();

            let naxis1 = *xtension.get_naxisn(1).unwrap() as usize;
            let naxis2 = *xtension.get_naxisn(2).unwrap() as usize;

            let num_pixels = naxis2 * naxis1;

            match xhdu.get_data_mut() {
                stream::Data::U8(st) => {
                    let data = st.collect::<Vec<_>>().await;
                    assert_eq!(num_pixels, data.len())
                },
                stream::Data::I16(st) => {
                    let data = st.collect::<Vec<_>>().await;
                    assert_eq!(num_pixels, data.len())
                },
                stream::Data::I32(st) => {
                    let data = st.collect::<Vec<_>>().await;
                    assert_eq!(num_pixels, data.len())
                },
                stream::Data::I64(st) => {
                    let data = st.collect::<Vec<_>>().await;
                    assert_eq!(num_pixels, data.len())
                },
                stream::Data::F32(st) => {
                    let data = st.collect::<Vec<_>>().await;
                    assert_eq!(num_pixels, data.len())
                },
                stream::Data::F64(st) => {
                    let data = st.collect::<Vec<_>>().await;
                    assert_eq!(num_pixels, data.len())
                },
            }
        },
        AsyncXtensionHDU::BinTable(xhdu) => {
            let num_bytes = xhdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let it_bytes = xhdu.get_data_mut();
            let data = it_bytes.collect::<Vec<_>>().await;
            assert_eq!(num_bytes as usize, data.len());
        },
        AsyncXtensionHDU::AsciiTable(xhdu) => {
            let num_bytes = xhdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let it_bytes = xhdu.get_data_mut();
            let data = it_bytes.collect::<Vec<_>>().await;
            assert_eq!(num_bytes as usize, data.len());
        },
    }

    hdu_ext = xhdu.next().await;
}
```
