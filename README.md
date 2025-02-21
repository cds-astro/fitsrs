FITS file reader written in pure Rust
-------------------------------------

[![](https://img.shields.io/crates/v/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![](https://img.shields.io/crates/d/fitsrs.svg)](https://crates.io/crates/fitsrs)
[![API Documentation on docs.rs](https://docs.rs/fitsrs/badge.svg)](https://docs.rs/fitsrs/)
![testing CI](https://github.com/cds-astro/fitsrs/actions/workflows/rust.yml/badge.svg)

This parser was initiated for reading FITS images mapped onto HEALPix cells in the sky (See the [HiPS IVOA](https://www.ivoa.net/documents/HiPS/) standard) in order to use it in the [Aladin Lite](https://github.com/cds-astro/aladin-lite) web sky atlas.

Currently, fitsrs supports reading multiple HDU and is mainly dedicated to image extension reading.
For interpreting WCS keywords, see [wcs-rs](https://github.com/cds-astro/wcs-rs).
A very new support of binary table extension has been added. This has been done mainly for supporting the [tiled compressed image convention](https://fits.gsfc.nasa.gov/registry/tilecompression.html) that describes the storing of tile images in variable length arrays of a binary table.
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

Features
--------

* [X] Support single typed data block (i.e. image type data)
* [X] Single HDU parsing, header and data units 
* [X] Support FITS files that may not fit in memory (iterator, possibility to seek directly to a specific pixel index/row)
* [X] Async reading (requires to read the whole data. Seeking is not possible)
* [X] Keeping COMMENTS, HISTORY and cards in the same order.
* [X] CONTINUE Long String Keyword convention
* [X] Keep all the cards in the original order
* [X] Basic support of Bintable
* [X] Tiled image convention for storing compressed images in FITS binary tables
    - [X] Compression supported, GZIP, GZIP2 and RICE on u8, i16, i32 and f32.
    - [ ] H_compress and PLI0 compressions
    - [X] Dithering techniques for floating point images. Not well tested (test samples are welcome)
    - [ ] `NULL_PIXEL_MASK` column and `ZMASKCMP` keyword is not supported
* [ ] FITS writer/serializer
* [ ] ESO HIERARCH keyword convention
* [ ] ASCII table extension parsing
* [X] Support of multiple HDU. Image and binary tables extension support. Provide an idiomatic Rust iterator over the list of HDU.
* [X] WCS parsing, see [wcs-rs](https://github.com/cds-astro/wcs-rs)
    - [X] Simple Imaging Polynomial (SIP) supported but not well tested
    - [ ] TNX, TPV, ZPX (non-standard conventions)

> [!NOTE]
> Features not done are not planned to be done. If you want fitsrs to support a specific convention, please open an issue or send us a mail to inform us of your use case(s) and we can manage to support them. The FITS standard and its conventions are massive and it is a huge work to support all of it.


License
-------

fitsrs has the double license MIT/Apache-2.0.

It uses code adapted from the famous [CFITSIO](https://github.com/HEASARC/cfitsio/blob/main/licenses/License.txt) library. Especially the RICE decompression source code has been ported from the original cfitsio [code](https://github.com/HEASARC/cfitsio/blob/main/ricecomp.c) to Rust.

Example
----------

```rust
use std::fs::File;
use std::io::Cursor;
use fitsrs::{Fits, ImageData, HDU, hdu::header::Xtension};
use fitsrs::wcs::{ImgXY, LonLat};

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

            // Try to access the WCS for an HDU image
            if let Ok(wcs) = hdu.wcs() {
                // Get the lonlat position on the sky of the pixel located at (0, 0) on the image
                let xy = ImgXY::new(0.0, 0.0);
                let lonlat = wcs
                    .unproj_lonlat(&xy)
                    .unwrap();

                // Get the pixel position in the image of a sky location
                let xy_2 = wcs
                    .proj_lonlat(&lonlat)
                    .unwrap();

                assert!((xy.x() - xy_2.x()).abs() <= 1e-9);
                assert!((xy.y() - xy_2.y()).abs() <= 1e-9);
            }
           
            match hdu_list.get_data(&hdu) {
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
            /*let data: Vec<_> = hdu_list
                .get_data(&hdu)
                .table_data()
                // Select specific fields with the select_fields method
                .select_fields(&[
                    ColumnId::Name("mag"),
                    ColumnId::Name("phot_bp_mean_mag"),
                    ColumnId::Name("phot_rp_mean_mag"),
                ])
                .collect();
            */
            let num_rows = hdu.get_header()
                .get_xtension()
                .get_num_rows();
            let rows: Vec<_> = hdu_list
                .get_data(&hdu)
                .row_iter()
                .collect();

            assert_eq!(num_rows, rows.len());
        },
        HDU::XASCIITable(hdu) => {
            let num_bytes = hdu.get_header()
                .get_xtension()
                .get_num_bytes_data_block();

            let data = hdu_list.get_data(&hdu)
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
