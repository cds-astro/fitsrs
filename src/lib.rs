//! This crate implements a fits image parser
//!
//! # Examples
//!
//! Basic usage:
//!
//! ```
//! use std::fs::File;
//! use std::io::BufReader;
//! 
//! use fitsrs::{fits::Fits, hdu::HDU};
//! use fitsrs::hdu::data::image::DataOwned;
//! 
//! let f = File::open("samples/fits.gsfc.nasa.gov/HST_FOC.fits").unwrap();
//! let mut reader = BufReader::new(f);
//! let Fits { mut hdu } = Fits::from_reader(&mut reader).unwrap();
//! let xtension = hdu.get_header().get_xtension();
//! let naxis1 = *xtension.get_naxisn(1).unwrap();
//! let naxis2 = *xtension.get_naxisn(2).unwrap();
//! 
//! if let DataOwned::F32(it) = hdu.get_data_mut() {
//!     let data = it.collect::<Vec<_>>();
//!     assert_eq!(data.len(), naxis1 * naxis2);
//! } else {
//!     panic!("expected data block containing f32");
//! }
//! ```

extern crate nom;
extern crate byteorder;
extern crate async_trait;

pub mod hdu;
pub mod fits;
pub mod card;
pub mod error;

#[cfg(test)]
mod tests {
    use crate::fits::{Fits, AsyncFits};
    use crate::hdu::extension::AsyncXtensionHDU;
    use crate::hdu::header::BitpixValue;
    use crate::hdu::data::image::{InMemData, DataOwned, AsyncDataOwned};
    use crate::hdu::{extension::XtensionHDU};
    use crate::hdu::header::extension::Xtension;

    use std::io::{Read, BufReader};
    use std::io::Cursor;
    use std::fs::File;

    use futures::StreamExt;
    use test_case::test_case;

    #[test]
    fn test_fits_image_mandatory_kw() {
        let f = File::open("samples/hipsgen/Npix208.fits").unwrap();
        let bytes: Result<Vec<_>, _> = f.bytes().collect();
        let buf = bytes.unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        let header = hdu.get_header();
        assert_eq!(header.get_xtension().get_naxisn(1), Some(&64));
        assert_eq!(header.get_xtension().get_naxisn(2), Some(&64));
        assert_eq!(header.get_xtension().get_naxis(), 2);
        assert_eq!(header.get_xtension().get_bitpix(), BitpixValue::F32);
        if let Ok(None) = hdu.next() {
            assert!(true);
        } else {
            assert!(false);
        }
    }

    #[test_case("samples/fits.gsfc.nasa.gov/Astro_UIT.fits", 1, 0, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/EUVE.fits", 5, 0, 4)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FGS.fits", 1, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOC.fits", 1, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOS.fits", 1, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_HRS.fits", 1, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_NICMOS.fits", 6, 0, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_WFPC_II.fits", 1, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_WFPC_II_bis.fits", 1, 0, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/IUE_LWP.fits", 1, 0, 1)]
    fn test_count_hdu(filename: &str, num_image_ext: usize, num_asciitable_ext: usize, num_bintable_ext: usize) {
        let f = File::open(filename).unwrap();
        let bytes: Result<Vec<_>, _> = f.bytes().collect();
        let buf = bytes.unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        let mut n_image_ext = 1; // because the primary hdu is an image
        let mut n_bintable_ext = 0;
        let mut n_asciitable_ext = 0;

        let mut hdu_ext = hdu.next();

        while let Ok(Some(hdu)) = hdu_ext {
            match &hdu {
                XtensionHDU::Image(_) => {
                    n_image_ext += 1;
                },
                XtensionHDU::BinTable(_) => {
                    n_bintable_ext += 1;
                },
                XtensionHDU::AsciiTable(_) => {
                    n_asciitable_ext += 1;
                },
            }

            hdu_ext = hdu.next();
        }

        assert_eq!(n_image_ext, num_image_ext);
        assert_eq!(n_bintable_ext, num_bintable_ext);
        assert_eq!(n_asciitable_ext, num_asciitable_ext);
    }
    
    #[test]
    fn test_fits_image_f32() {
        let f = File::open("samples/hipsgen/Npix208.fits").unwrap();
        let bytes: Result<Vec<_>, _> = f.bytes().collect();
        let buf = bytes.unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        let header = hdu.get_header();
        let num_pixels = header.get_xtension().get_naxisn(1).unwrap() * header.get_xtension().get_naxisn(2).unwrap();
        let data = hdu.get_data();
        match data {
            InMemData::F32(slice) => {
                assert!(slice.len() == num_pixels);
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_fits_i16() {
        let mut f = File::open("samples/hipsgen/Npix4906.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let mut reader = Cursor::new(&raw_bytes[..]);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();
        let xtension = hdu.get_header().get_xtension();
        match hdu.get_data() {
            &InMemData::I16(data) => {
                assert!(data.len() == xtension.get_naxisn(1).unwrap() * xtension.get_naxisn(2).unwrap())
            },
            _ => unreachable!(),
        }
    }

    #[test_case("samples/hipsgen/Npix8.fits")]
    #[test_case("samples/hipsgen/Npix9.fits")]
    #[test_case("samples/hipsgen/Npix132.fits")]
    #[test_case("samples/hipsgen/Npix133.fits")]
    #[test_case("samples/hipsgen/Npix134.fits")]
    #[test_case("samples/hipsgen/Npix140.fits")]
    #[test_case("samples/hipsgen/Npix208.fits")]
    #[test_case("samples/hipsgen/Npix282.fits")]
    #[test_case("samples/hipsgen/Npix4906.fits")]
    #[test_case("samples/hipsgen/Npix691539.fits")]
    #[test_case("samples/hips2fits/allsky_panstarrs.fits")]
    #[test_case("samples/hips2fits/cutout-CDS_P_HST_PHAT_F475W.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/Astro_UIT.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/EUVE.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FGS.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOC.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOS.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_HRS.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_NICMOS.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_WFPC_II.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_WFPC_II_bis.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/IUE_LWP.fits")]
    #[test_case("samples/misc/bonn.fits")]
    #[test_case("samples/misc/P122_49.fits")]
    #[test_case("samples/misc/skv1678175163788.fits")]
    //#[test_case("samples/misc/drao.fits")] png file
    //#[test_case("samples/misc/ji0590044.fits")] gzip compressed data
    //#[test_case("samples/misc/AKAI013000932.fits")] gzip compressed data
    fn test_fits_opening(filename: &str) {
        use std::fs::File;

        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let fits = Fits::from_reader(&mut reader);
        assert!(fits.is_ok());
    }

    #[test]
    fn test_fits_image_owned() {
        use std::fs::File;
        use std::io::BufReader;

        let f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
        let mut reader = BufReader::new(f);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        if let Ok(Some(XtensionHDU::Image(mut image))) = hdu.next() {
            let xtension = image.get_header().get_xtension();
            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();

            let data = image.get_data_mut();
            match data {
                DataOwned::I16(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(data.len(), naxis1 * naxis2);
                },
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_fits_image_borrowed() {
        use std::fs::File;

        let mut f = File::open("samples/fits.gsfc.nasa.gov/HST_FOC.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        let naxis1 = *hdu.get_header().get_xtension().get_naxisn(1).unwrap();
        let naxis2 = *hdu.get_header().get_xtension().get_naxisn(2).unwrap();
        let data = hdu.get_data();
        match data {
            &InMemData::F32(data) => {
                assert_eq!(data.len(), naxis1 * naxis2);
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_fits_images_data_block() {
        use std::fs::File;

        let mut f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let mut reader = Cursor::new(&buf[..]);

        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        let mut hdu_ext = hdu.next();

        while let Ok(Some(xhdu)) = hdu_ext {
            match &xhdu {
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

            hdu_ext = xhdu.next();
        }
    }

    #[test]
    fn test_fits_images_data_block_bufreader() {
        use std::fs::File;

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

            hdu_ext = xhdu.next();
        }
    }

    #[test]
    fn test_bad_bytes() {
        let bytes: &[u8] = &[
            60, 33, 68, 79, 67, 84, 89, 80, 69, 32, 72, 84, 77, 76, 32, 80, 85, 66, 76, 73, 67, 32,
            34, 45, 47, 47, 73, 69, 84, 70, 47, 47, 68, 84, 68, 32, 72, 84, 77, 76, 32, 50, 46, 48,
            47, 47, 69, 78, 34, 62, 10, 60, 104, 116, 109, 108, 62, 60, 104, 101, 97, 100, 62, 10,
            60, 116, 105, 116, 108, 101, 62, 52, 48, 52, 32, 78, 111, 116, 32, 70, 111, 117, 110,
            100, 60, 47, 116, 105, 116, 108, 101, 62, 10, 60, 47, 104, 101, 97, 100, 62, 60, 98,
            111, 100, 121, 62, 10, 60, 104, 49, 62, 78, 111, 116, 32, 70, 111, 117, 110, 100, 60,
            47, 104, 49, 62, 10, 60, 112, 62, 84, 104, 101, 32, 114, 101, 113, 117, 101, 115, 116,
            101, 100, 32, 85, 82, 76, 32, 47, 97, 108, 108, 115, 107, 121, 47, 80, 78, 82, 101,
            100, 47, 78, 111, 114, 100, 101, 114, 55, 47, 68, 105, 114, 52, 48, 48, 48, 48, 47, 78,
            112, 105, 120, 52, 52, 49, 49, 49, 46, 102, 105, 116, 115, 32, 119, 97, 115, 32, 110,
            111, 116, 32, 102, 111, 117, 110, 100, 32, 111, 110, 32, 116, 104, 105, 115, 32, 115,
            101, 114, 118, 101, 114, 46, 60, 47, 112, 62, 10, 60, 47, 98, 111, 100, 121, 62, 60,
            47, 104, 116, 109, 108, 62, 10,
        ];
        let mut reader = Cursor::new(bytes);
        assert!(Fits::from_reader(&mut reader).is_err());
    }

    #[tokio::test]
    async fn test_fits_images_data_block_bufreader_async() {
        use std::fs::File;

        // Put it all in memory first (this is for the exemple)
        // It is not good to do so for performance reasons
        // Better prefer to pipe to a ReadableStream instead
        let mut f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = futures::io::BufReader::new(&buf[..]);

        let AsyncFits { hdu } = AsyncFits::from_reader(&mut reader).await.unwrap();

        let mut hdu_ext = hdu.next().await;

        while let Ok(Some(mut xhdu)) = hdu_ext {
            match &mut xhdu {
                AsyncXtensionHDU::Image(xhdu) => {
                    let xtension = xhdu.get_header().get_xtension();

                    let naxis1 = *xtension.get_naxisn(1).unwrap();
                    let naxis2 = *xtension.get_naxisn(2).unwrap();

                    let num_pixels = naxis2 * naxis1;

                    match xhdu.get_data_mut() {
                        AsyncDataOwned::U8(stream) => {
                            let data = stream.collect::<Vec<_>>().await;
                            assert_eq!(num_pixels, data.len())
                        },
                        AsyncDataOwned::I16(stream) => {
                            let data = stream.collect::<Vec<_>>().await;
                            assert_eq!(num_pixels, data.len())
                        },
                        AsyncDataOwned::I32(stream) => {
                            let data = stream.collect::<Vec<_>>().await;
                            assert_eq!(num_pixels, data.len())
                        },
                        AsyncDataOwned::I64(stream) => {
                            let data = stream.collect::<Vec<_>>().await;
                            assert_eq!(num_pixels, data.len())
                        },
                        AsyncDataOwned::F32(stream) => {
                            let data = stream.collect::<Vec<_>>().await;
                            assert_eq!(num_pixels, data.len())
                        },
                        AsyncDataOwned::F64(stream) => {
                            let data = stream.collect::<Vec<_>>().await;
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
                    assert_eq!(num_bytes, data.len());
                },
                AsyncXtensionHDU::AsciiTable(xhdu) => {
                    let num_bytes = xhdu.get_header()
                        .get_xtension()
                        .get_num_bytes_data_block();

                    let it_bytes = xhdu.get_data_mut();
                    let data = it_bytes.collect::<Vec<_>>().await;
                    assert_eq!(num_bytes, data.len());
                },
            }

            hdu_ext = xhdu.next().await;
        }
    }
}
