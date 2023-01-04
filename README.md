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

```rust
use std::fs::File;

use std::io::prelude::*;

extern crate fitsrs;
use fitsrs::{Fits, DataType};

fn main() {
    let mut f = File::open("../fitsreader/misc/allsky_panstarrs.fits").unwrap();
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer).unwrap();

    let Fits { header: _header, data } = Fits::from_byte_slice(&buffer.as_slice()).unwrap();
    match data {
        DataType::F32(_v) => {
            // v
        },
        _ => ()
    }
}
```