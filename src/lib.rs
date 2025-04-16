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
//! use fitsrs::{Fits, HDU, ImageData, Pixels};
//!
//! let f = File::open("samples/fits.gsfc.nasa.gov/HST_FOC.fits").unwrap();
//! let reader = BufReader::new(f);
//! let mut hdu_list = Fits::from_reader(reader);
//! let hdu = hdu_list.next();
//! if let Some(Ok(HDU::Primary(hdu))) = hdu {
//!     let xtension = hdu.get_header().get_xtension();
//!     let naxis1 = *xtension.get_naxisn(1).unwrap() as usize;
//!     let naxis2 = *xtension.get_naxisn(2).unwrap() as usize;
//!
//!     let image = hdu_list.get_data(&hdu);
//!     if let Pixels::F32(it) = image.pixels() {
//!         let data = it.collect::<Vec<_>>();
//!         assert_eq!(data.len(), naxis1 * naxis2);
//!     } else {
//!         panic!("expected data block containing f32");
//!     }
//! }
//! ```

#![doc = include_str!("../README.md")]
#![warn(trivial_casts, trivial_numeric_casts, clippy::uninlined_format_args, clippy::match_same_arms)]

extern crate async_trait;
//extern crate byteorder;
#[macro_use]
extern crate quick_error;

pub mod async_fits;
pub mod card;
pub mod error;
pub mod file;
pub mod fits;
pub mod wcs;

mod gz;
pub mod hdu;

pub use async_fits::AsyncFits;
pub use file::FITSFile;
pub use fits::Fits;
pub use hdu::data::bintable::{BinaryTableData, DataValue, TableData, TableRowData};
pub use hdu::data::image::{ImageData, Pixels};
pub use hdu::data::iter::It;
pub use hdu::{AsyncHDU, HDU};

#[cfg(test)]
mod tests {
    use crate::async_fits::AsyncFits;
    use crate::fits::Fits;
    use crate::hdu::data::image::Pixels;
    use crate::hdu::data::DataStream;
    use crate::hdu::AsyncHDU;
    use crate::wcs::ImgXY;
    use crate::FITSFile;

    use crate::hdu::data::bintable::ColumnId;
    use crate::hdu::header::extension::Xtension;
    use crate::hdu::header::Bitpix;

    use std::fs::File;
    use std::io::Cursor;
    use std::io::{BufReader, Read};

    use futures::StreamExt;
    use image::DynamicImage;
    use test_case::test_case;

    #[test]
    fn test_fits_image_mandatory_kw() {
        let f = File::open("samples/hipsgen/Npix208.fits").unwrap();
        let bytes: Result<Vec<_>, _> = f.bytes().collect();
        let buf = bytes.unwrap();

        let reader = Cursor::new(&buf[..]);
        let mut hdu_list = Fits::from_reader(reader);
        let hdu = hdu_list.next().unwrap().unwrap();
        assert!(matches!(hdu, HDU::Primary(_)));

        if let HDU::Primary(hdu) = hdu {
            let header = hdu.get_header();
            assert_eq!(header.get_xtension().get_naxisn(1), Some(&64));
            assert_eq!(header.get_xtension().get_naxisn(2), Some(&64));
            assert_eq!(header.get_xtension().get_naxis(), 2);
            assert_eq!(header.get_xtension().get_bitpix(), Bitpix::F32);
        }

        assert!(hdu_list.next().is_none());
    }

    #[test_case("samples/fits.gsfc.nasa.gov/Astro_UIT.fits", 2, 0, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/EUVE.fits", 6, 0, 4)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FGS.fits", 2, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/IUE_LWP.fits", 2, 0, 1)]
    #[test_case("samples/misc/ngc5457K.fits", 2, 0, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOC.fits", 2, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOS.fits", 2, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_HRS.fits", 2, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_NICMOS.fits", 7, 0, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_WFPC_II.fits", 2, 1, 0)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_WFPC_II_bis.fits", 2, 0, 0)]
    fn test_fits_count_hdu(
        filename: &str,
        num_image_ext: usize,
        num_asciitable_ext: usize,
        num_bintable_ext: usize,
    ) {
        let mut hdu_list = FITSFile::open(filename).unwrap();

        let mut n_image_ext = 1; // because the primary hdu is an image
        let mut n_bintable_ext = 0;
        let mut n_asciitable_ext = 0;

        while let Some(Ok(hdu)) = hdu_list.next() {
            match &hdu {
                HDU::Primary(_) | HDU::XImage(_) => {
                    n_image_ext += 1;
                }
                HDU::XBinaryTable(_) => {
                    n_bintable_ext += 1;
                }
                HDU::XASCIITable(_) => {
                    n_asciitable_ext += 1;
                }
            }
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

        let reader = Cursor::new(&buf[..]);
        let mut hdu_list = Fits::from_reader(reader);

        let hdu = hdu_list.next().unwrap().unwrap();
        assert!(matches!(hdu, HDU::Primary(_)));
        if let HDU::Primary(hdu) = hdu {
            let header = hdu.get_header();
            let num_pixels = header.get_xtension().get_naxisn(1).unwrap()
                * header.get_xtension().get_naxisn(2).unwrap();
            let image = hdu_list.get_data(&hdu);
            match image.pixels() {
                Pixels::F32(it) => {
                    assert!(it.collect::<Vec<_>>().len() as u64 == num_pixels);
                }
                _ => unreachable!(),
            }
        } else {
            unreachable!();
        }
    }

    #[test]
    fn test_fits_i16() {
        let mut f = File::open("samples/hipsgen/Npix4906.fits").unwrap();
        let mut raw_bytes = Vec::<u8>::new();
        f.read_to_end(&mut raw_bytes).unwrap();

        let reader = Cursor::new(&raw_bytes[..]);
        let mut hdu_list = Fits::from_reader(reader);

        let hdu = hdu_list.next().unwrap().unwrap();
        assert!(matches!(hdu, HDU::Primary(_)));
        if let HDU::Primary(hdu) = hdu {
            let header = hdu.get_header();
            let num_pixels = header.get_xtension().get_naxisn(1).unwrap()
                * header.get_xtension().get_naxisn(2).unwrap();
            let image = hdu_list.get_data(&hdu);
            match image.pixels() {
                Pixels::I16(data) => {
                    assert!(data.collect::<Vec<_>>().len() as u64 == num_pixels)
                }
                _ => unreachable!(),
            }
        } else {
            unreachable!();
        }
    }

    #[test_case("samples/fits.gsfc.nasa.gov/Astro_UIT.fits", true)]
    #[test_case("samples/hipsgen/Npix8.fits", false)]
    #[test_case("samples/hipsgen/Npix9.fits", false)]
    #[test_case("samples/hipsgen/Npix132.fits", false)]
    #[test_case("samples/hipsgen/Npix133.fits", false)]
    #[test_case("samples/hipsgen/Npix134.fits", false)]
    #[test_case("samples/hipsgen/Npix140.fits", false)]
    #[test_case("samples/hipsgen/Npix208.fits", false)]
    #[test_case("samples/hipsgen/Npix282.fits", false)]
    #[test_case("samples/hipsgen/Npix4906.fits", false)]
    #[test_case("samples/hipsgen/Npix691539.fits", false)]
    #[test_case("samples/hips2fits/allsky_panstarrs.fits", false)]
    #[test_case("samples/hips2fits/cutout-CDS_P_HST_PHAT_F475W.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/EUVE.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FGS.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOC.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOS.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_HRS.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_NICMOS.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_WFPC_II.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_WFPC_II_bis.fits", false)]
    #[test_case("samples/vizier/NVSSJ235137-362632r.fits", false)]
    #[test_case("samples/vizier/VAR.358.R.fits", false)]
    #[test_case("samples/fits.gsfc.nasa.gov/IUE_LWP.fits", false)]
    #[test_case("samples/misc/bonn.fits", false)]
    #[test_case("samples/misc/EUC_MER_MOSAIC-VIS-FLAG_TILE100158585-1EC1C5_20221211T132329.822037Z_00.00.fits", false)]
    #[test_case("samples/misc/P122_49.fits", false)]
    #[test_case("samples/misc/skv1678175163788.fits", false)]
    #[test_case("samples/misc/SN2923fxjA.fits", false)]
    fn test_fits_opening(filename: &str, ground_truth: bool) {
        let hdu_list = FITSFile::open(filename).expect("Can find fits file");

        let mut corrupted = false;
        for hdu in hdu_list {
            if hdu.is_err() {
                corrupted = true;
            }
        }

        assert_eq!(ground_truth, corrupted);
    }

    #[test]
    fn test_fits_not_fitting_in_memory() {
        use std::fs::File;
        use std::io::BufReader;
        let f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
        let reader = BufReader::new(f);
        let mut hdu_list = Fits::from_reader(reader);

        while let Some(Ok(HDU::XImage(hdu))) = hdu_list.next() {
            let xtension = hdu.get_header().get_xtension();
            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();

            let num_pixels = (naxis1 * naxis2) as usize;

            // Try to access the WCS on a specific HDU image
            if let Ok(wcs) = hdu.wcs() {
                // and perform projection/unprojection using that image WCS
                let xy = ImgXY::new(0.0, 0.0);
                let _lonlat = wcs.unproj_lonlat(&xy).unwrap();
            }

            let image = hdu_list.get_data(&hdu);
            match image.pixels() {
                Pixels::I16(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(data.len(), num_pixels);
                }
                Pixels::U8(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(data.len(), num_pixels);
                }
                Pixels::I32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(data.len(), num_pixels);
                }
                Pixels::I64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(data.len(), num_pixels);
                }
                Pixels::F32(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(data.len(), num_pixels);
                }
                Pixels::F64(it) => {
                    let data = it.collect::<Vec<_>>();
                    assert_eq!(data.len(), num_pixels);
                }
            }
        }
    }

    #[test]
    fn test_fits_image_borrowed() {
        use std::fs::File;

        let mut f = File::open("samples/fits.gsfc.nasa.gov/HST_FOC.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let reader = Cursor::new(&buf[..]);
        let mut hdu_list = Fits::from_reader(reader);

        if let Some(Ok(HDU::Primary(hdu))) = hdu_list.next() {
            let xtension = hdu.get_header().get_xtension();
            let naxis1 = *xtension.get_naxisn(1).unwrap();
            let naxis2 = *xtension.get_naxisn(2).unwrap();
            let image = hdu_list.get_data(&hdu);
            match image.pixels() {
                Pixels::F32(data) => {
                    assert_eq!(data.collect::<Vec<_>>().len(), (naxis1 * naxis2) as usize);
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn test_fits_bintable() {
        use std::fs::File;

        let f = File::open("samples/vizier/II_278_transit.fits").unwrap();

        let reader = BufReader::new(f);
        let mut hdu_list = Fits::from_reader(reader);
        let mut data = vec![];
        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let _ = hdu.get_header().get_xtension();
                data = hdu_list.get_data(&hdu).collect::<Vec<_>>();
            }
        }

        assert_eq!(177 * 23, data.len());
    }

    #[test]
    fn test_fits_bintable_corr() {
        use std::fs::File;

        let f = File::open("samples/astrometry.net/corr.fits").unwrap();

        let reader = BufReader::new(f);
        let mut hdu_list = Fits::from_reader(reader);
        while let Some(Ok(hdu)) = hdu_list.next() {
            if let HDU::XBinaryTable(hdu) = hdu {
                let data: Vec<_> = hdu_list
                    .get_data(&hdu)
                    .table_data()
                    .select_fields(&[
                        ColumnId::Name("mag"),
                        ColumnId::Name("phot_bp_mean_mag"),
                        ColumnId::Name("phot_rp_mean_mag"),
                    ])
                    .collect();

                assert_eq!(data.len(), 3 * 52);
            }
        }
    }

    #[test_case("samples/misc/SN2923fxjA.fits.gz", 5415.0, 6386.0)]
    #[test_case("samples/misc/SN2923fxjA.fits", 5415.0, 6386.0)]
    fn test_fits_open_external_gzipped_file(filename: &str, min: f32, max: f32) {
        let mut hdu_list = FITSFile::open(filename).unwrap();
        use std::iter::Iterator;

        while let Some(Ok(hdu)) = hdu_list.next() {
            match hdu {
                HDU::Primary(hdu) | HDU::XImage(hdu) => {
                    let xtension = hdu.get_header().get_xtension();
                    let naxis1 = *xtension.get_naxisn(1).unwrap();
                    let naxis2 = *xtension.get_naxisn(2).unwrap();

                    let image = hdu_list.get_data(&hdu);
                    if let Pixels::F32(it) = image.pixels() {
                        let c = it
                            .map(|v| (((v - min) / (max - min)) * 255.0) as u8)
                            .collect::<Vec<_>>();

                        let imgbuf = DynamicImage::ImageLuma8(
                            image::ImageBuffer::from_raw(naxis1 as u32, naxis2 as u32, c).unwrap(),
                        );
                        imgbuf.save(format!("{filename}.jpg")).unwrap();
                    };
                }
                _ => (),
            }
        }
    }

    use super::hdu::HDU;
    #[test]
    fn test_fits_images_data_block() {
        use std::fs::File;

        let mut f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let reader = Cursor::new(&buf[..]);

        let mut hdu_list = Fits::from_reader(reader);

        while let Some(Ok(hdu)) = hdu_list.next() {
            match hdu {
                HDU::XImage(hdu) | HDU::Primary(hdu) => {
                    let xtension = hdu.get_header().get_xtension();

                    let naxis1 = xtension.get_naxisn(1);
                    let naxis2 = xtension.get_naxisn(2);

                    if let (Some(naxis1), Some(naxis2)) = (naxis1, naxis2) {
                        let num_pixels = (naxis2 * naxis1) as usize;

                        let image = hdu_list.get_data(&hdu);
                        match image.pixels() {
                            Pixels::U8(it) => {
                                assert_eq!(num_pixels, it.collect::<Vec<_>>().len())
                            }
                            Pixels::I16(it) => {
                                assert_eq!(num_pixels, it.collect::<Vec<_>>().len())
                            }
                            Pixels::I32(it) => {
                                assert_eq!(num_pixels, it.collect::<Vec<_>>().len())
                            }
                            Pixels::I64(it) => {
                                assert_eq!(num_pixels, it.collect::<Vec<_>>().len())
                            }
                            Pixels::F32(it) => {
                                assert_eq!(num_pixels, it.collect::<Vec<_>>().len())
                            }
                            Pixels::F64(it) => {
                                assert_eq!(num_pixels, it.collect::<Vec<_>>().len())
                            }
                        }
                    };
                }
                HDU::XBinaryTable(hdu) => {
                    let _num_bytes = hdu.get_header().get_xtension().get_num_bytes_data_block();
                    let _data = hdu_list.get_data(&hdu);
                    /*{
                        It(mem) => assert_eq!(num_bytes as usize, mem.len()),
                        _ => unreachable!(),
                    }*/
                }
                HDU::XASCIITable(hdu) => {
                    let num_bytes = hdu.get_header().get_xtension().get_num_bytes_data_block();
                    let bytes = hdu_list.get_data(&hdu);

                    assert_eq!(num_bytes as usize, bytes.collect::<Vec<_>>().len());
                }
            }
        }
    }

    #[test]
    fn test_fits_images_data_block_bufreader() {
        use std::fs::File;

        let f = File::open("samples/fits.gsfc.nasa.gov/EUVE.fits").unwrap();
        let reader = BufReader::new(f);

        let mut hdu_list = Fits::from_reader(reader);

        while let Some(Ok(hdu)) = hdu_list.next() {
            match hdu {
                HDU::XImage(hdu) => {
                    let xtension = hdu.get_header().get_xtension();

                    let naxis1 = *xtension.get_naxisn(1).unwrap();
                    let naxis2 = *xtension.get_naxisn(2).unwrap();

                    let num_pixels = naxis2 * naxis1;

                    let image = hdu_list.get_data(&hdu);
                    match image.pixels() {
                        Pixels::U8(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels as usize, data.len())
                        }
                        Pixels::I16(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels as usize, data.len())
                        }
                        Pixels::I32(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels as usize, data.len())
                        }
                        Pixels::I64(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels as usize, data.len())
                        }
                        Pixels::F32(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels as usize, data.len())
                        }
                        Pixels::F64(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels as usize, data.len())
                        }
                    }
                }
                HDU::XBinaryTable(_) => {
                    /*let num_bytes = hdu.get_header().get_xtension().get_num_bytes_data_block();

                    let it_bytes = hdu.get_data(&mut hdu_list);
                    let data = it_bytes.collect::<Vec<_>>();
                    assert_eq!(num_bytes as usize, data.len());*/
                }
                HDU::XASCIITable(hdu) => {
                    let num_bytes = hdu.get_header().get_xtension().get_num_bytes_data_block();

                    let it_bytes = hdu_list.get_data(&hdu);
                    let data = it_bytes.collect::<Vec<_>>();
                    assert_eq!(num_bytes as usize, data.len());
                }
                _ => (),
            }
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
        let reader = Cursor::new(bytes);
        let mut hdu_list = Fits::from_reader(reader);
        assert!(hdu_list.next().unwrap().is_err());
    }

    // FIXME too slow, to retest when we implement the seek of the data unit part
    //#[test_case("samples/misc/EUC_MER_MOSAIC-VIS-FLAG_TILE100158585-1EC1C5_20221211T132329.822037Z_00.00.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/EUVE.fits")]
    #[test_case("samples/fits.gsfc.nasa.gov/HST_FOC.fits")]
    #[test_case("samples/vizier/new_url.fits")]
    #[tokio::test]
    async fn test_fits_images_data_block_bufreader_async(filename: &str) {
        // Put it all in memory first (this is for the exemple)
        // It is not good to do so for performance reasons
        // Better prefer to pipe to a ReadableStream instead
        let mut f = File::open(filename).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let reader = futures::io::BufReader::new(&buf[..]);

        let mut hdu_list = AsyncFits::from_reader(reader);

        while let Some(Ok(hdu)) = hdu_list.next().await {
            match hdu {
                AsyncHDU::XImage(hdu) | AsyncHDU::Primary(hdu) => {
                    let xtension = hdu.get_header().get_xtension();
                    let naxis1 = xtension.get_naxisn(1);
                    let naxis2 = xtension.get_naxisn(2);
                    if let (Some(naxis1), Some(naxis2)) = (naxis1, naxis2) {
                        let num_pixels = (*naxis2 * *naxis1) as usize;

                        match hdu_list.get_data(&hdu) {
                            DataStream::U8(st) => {
                                let data = st.collect::<Vec<_>>().await;
                                assert_eq!(num_pixels, data.len())
                            }
                            DataStream::I16(stream) => {
                                let data = stream.collect::<Vec<_>>().await;
                                assert_eq!(num_pixels, data.len())
                            }
                            DataStream::I32(stream) => {
                                let data = stream.collect::<Vec<_>>().await;
                                assert_eq!(num_pixels, data.len());
                            }
                            DataStream::I64(stream) => {
                                let data = stream.collect::<Vec<_>>().await;
                                assert_eq!(num_pixels, data.len())
                            }
                            DataStream::F32(stream) => {
                                let data = stream.collect::<Vec<_>>().await;
                                assert_eq!(num_pixels, data.len())
                            }
                            DataStream::F64(stream) => {
                                let data = stream.collect::<Vec<_>>().await;
                                assert_eq!(num_pixels, data.len())
                            }
                        }
                    }
                }
                AsyncHDU::XBinaryTable(hdu) => {
                    let num_bytes = hdu.get_header().get_xtension().get_num_bytes_data_block();

                    let it_bytes = hdu_list.get_data(&hdu);
                    let data = it_bytes.collect::<Vec<_>>().await;
                    assert_eq!(num_bytes as usize, data.len());
                }
                AsyncHDU::XASCIITable(hdu) => {
                    let num_bytes = hdu.get_header().get_xtension().get_num_bytes_data_block();

                    let it_bytes = hdu_list.get_data(&hdu);
                    let data = it_bytes.collect::<Vec<_>>().await;
                    assert_eq!(num_bytes as usize, data.len());
                }
            }
        }
    }

    #[test]
    fn test_fits_euve() {
        use std::fs::File;
        use std::io::BufReader;

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

                    let data = hdu_list.get_data(&hdu);
                    match data.pixels() {
                        Pixels::U8(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels, data.len())
                        }
                        Pixels::I16(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels, data.len())
                        }
                        Pixels::I32(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels, data.len())
                        }
                        Pixels::I64(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels, data.len())
                        }
                        Pixels::F32(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels, data.len())
                        }
                        Pixels::F64(it) => {
                            let data = it.collect::<Vec<_>>();
                            assert_eq!(num_pixels, data.len())
                        }
                    }
                }
                HDU::XBinaryTable(hdu) => {
                    let num_rows = hdu.get_header().get_xtension().get_num_rows();

                    let rows = hdu_list.get_data(&hdu).row_iter().collect::<Vec<_>>();

                    assert_eq!(num_rows, rows.len());
                }
                HDU::XASCIITable(hdu) => {
                    let num_bytes = hdu.get_header().get_xtension().get_num_bytes_data_block();

                    let data = hdu_list.get_data(&hdu).collect::<Vec<_>>();

                    assert_eq!(num_bytes as usize, data.len());
                }
            }
        }
    }
}
