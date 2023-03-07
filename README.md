Fits reader written in pure Rust using [nom](https://github.com/Geal/nom)
-------------------------------------------------------------------------

[![](https://img.shields.io/crates/v/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![](https://img.shields.io/crates/d/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![API Documentation on docs.rs](https://docs.rs/fitsrs/badge.svg)](https://docs.rs/fitsrs/)

This crate is under heavy development, it was initiated for reading fits HiPS tile, i.e. generated from hipsgen.

This fits parser only supports image data (not tables), and does not know anything about WCS parsing.
For WCS parsing, see [wcsrs](https://github.com/cds-astro/wcs-rs).
This parser also does not parse multiple HDUs, extensions. If a fits file containing multiple extensions is given to fitsrs, 
then only its first HDU will be parsed and the following ones will be ignored.

To Do list
----------

* [X] Support single typed data block (i.e. image type data)
* [X] Single HDU parsing, header and data units 
* [X] Support big fits file parsing that may not fit in memory (iterator usage)
* [X] Async reading (experimental and not tested)
* [ ] Keep CARD comment
* [ ] Support data table (each column can have a specific types)
* [ ] Support of multiple HDU, fits extensions
* [ ] WCS parsing, see [wcsrs](https://github.com/cds-astro/wcs-rs)

Example
----------

For files that can fit in memory
```rust
use std::fs::File;

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

            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();

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
                InMemData::U8(mem) => assert_eq!(num_bytes, mem.len()),
                _ => unreachable!()
            }
        },
        XtensionHDU::AsciiTable(xhdu) => {
            let num_bytes = xhdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            match xhdu.get_data() {
                InMemData::U8(mem) => assert_eq!(num_bytes, mem.len()),
                _ => unreachable!()
            }
        },
    }

    hdu_ext = hdu.next();
}
```

For files that may not be contained into the memory
```rust
let f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
let mut reader = BufReader::new(f);

let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

let mut hdu_ext = hdu.next();

while let Ok(Some(mut hdu)) = hdu_ext {
    match &mut hdu {
        XtensionHDU::Image(xhdu) => {
            let xtension = xhdu.get_header().get_xtension();

            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();

            let num_pixels = naxis2 * naxis1;

            match xhdu.get_data_mut() {
                DataOwned::U8(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataOwned::I16(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataOwned::I32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataOwned::I64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataOwned::F32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(num_pixels, data.len())
                },
                DataOwned::F64(it) => {
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
            assert_eq!(num_bytes, data.len());
        },
        XtensionHDU::AsciiTable(xhdu) => {
            let num_bytes = xhdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let it_bytes = xhdu.get_data_mut();
            let data = it_bytes.collect::<Vec<_>>();
            assert_eq!(num_bytes, data.len());
        },
    }

    hdu_ext = hdu.next();
}
```