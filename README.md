# FITS reader written in pure Rust using [nom](https://github.com/Geal/nom)

This crate is under heavy development, it was initiated for reading fits HiPS tile, i.e. generated from hipsgen and therefore taking into account that a fits tile:

* Contains all its data the primary unit
* Does not use any WCS

## What is supported ?

The read of a primary unit, i.e. the primary header and data unit
The extensions are not supported

## Exemple

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

To run the tests:
``
cargo test
``
