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
//! use fitsrs::{hdu::data::DataOwned, fits::Fits, hdu::HDU};
//! 
//! let f = File::open("misc/FOCx38i0101t_c0f.fits").unwrap();
//! let mut reader = BufReader::new(f);
//! let Fits { mut hdu } = Fits::from_reader(&mut reader).unwrap();
//! for HDU { header, data } in hdu {
//!     // Retrieve some card values
//!     let naxis1 = header.get_axis_size(1).unwrap();
//!     let naxis2 = header.get_axis_size(2).unwrap();
//! 
//!     // Get the data part iterator
//!     match data {
//!         // Knowing the BITPIX keyword you are able to know the correct data type
//!         DataOwned::F32(it) => {
//!             // Consume it when you want
//!             let data = it.collect::<Vec<_>>();
//!             assert_eq!(data.len(), naxis1 * naxis2);
//!         },
//!         _ => unreachable!(),
//!     }
//! }
//! ```

extern crate nom;
extern crate byteorder;

pub mod hdu;
pub mod fits;
pub mod card;
pub mod error;

#[cfg(test)]
mod tests {
    use crate::fits::Fits;
    use crate::hdu::header::BitpixValue;
    use crate::hdu::data::image::{InMemData};
    use crate::hdu::{extension::XtensionHDU};

    use std::io::Read;
    use std::io::Cursor;
    use std::fs::File;

    use test_case::test_case;

    #[test]
    fn test_fits_tile() {
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
        if let Ok(None) = dbg!(hdu.next()) {
            assert!(true)
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

        let mut hdu_ext = hdu.next();
        let mut n_image_ext = 1; // because the primary hdu is an image
        let mut n_bintable_ext = 0;
        let mut n_asciitable_ext = 0;

        while let Ok(Some(hdu)) = hdu_ext {
            match hdu {
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
    fn test_fits_f32() {
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

    /*#[test]
    fn test_fits_async() {
        use std::fs::File;
        use std::io::BufReader;

        let mut f = File::open("misc/Npix282.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        use futures::executor::LocalPool;

        let mut pool = LocalPool::new();

        // run tasks in the pool until `my_app` completes
        pool.run_until(async {
            let Fits { data, .. } = Fits::from_byte_slice_async(&buf[..]).await.unwrap();

            matches!(data, super::DataType::F32(_));
        });
    }*/

    /*#[test]
    fn test_fits_tile3() {
        use std::fs::File;

        let mut f = File::open("misc/Npix4906.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        let mut reader = Cursor::new(&buf[..]);

        let _fits = Fits::from_reader(&mut reader).unwrap();
    }

    #[test]
    fn test_fits_tile4() {
        use std::fs::File;

        let mut f = File::open("misc/Npix9.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let _fits = Fits::from_reader(&mut reader).unwrap();
    }

    #[test]
    fn test_fits_image() {
        use std::fs::File;

        let mut f = File::open("misc/cutout-CDS_P_HST_PHAT_F475W.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let _fits = Fits::from_reader(&mut reader).unwrap();
    }

    #[test]
    fn test_fits_image_owned() {
        use std::fs::File;
        use std::io::BufReader;

        let f = File::open("misc/FOCx38i0101t_c0f.fits").unwrap();
        let mut reader = BufReader::new(f);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        use crate::hdu::HDU;
        for HDU { data, header} in hdu {
            let naxis1 = header.get_axis_size(1).unwrap();
            let naxis2 = header.get_axis_size(2).unwrap();
    
            match data {
                DataOwned::F32(it) => {
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

        let mut f = File::open("misc/FOCx38i0101t_c0f.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        let Fits { hdu } = Fits::from_reader(&mut reader).unwrap();

        let header = &hdu[0].header;
        let naxis1 = header.get_axis_size(1).unwrap();
        let naxis2 = header.get_axis_size(2).unwrap();

        match hdu[0].data {
            DataBorrowed::F32(data) => {
                assert_eq!(data.len(), naxis1 * naxis2);
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_fits_tile5() {
        use std::fs::File;

        let mut f = File::open("misc/Npix133.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        Fits::from_reader(&mut reader).unwrap();
    }
    #[test]
    fn test_fits_tile6() {
        use std::fs::File;

        let mut f = File::open("misc/Npix8.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        Fits::from_reader(&mut reader).unwrap();
    }

    #[test]
    fn test_fits_tile7() {
        use std::fs::File;

        let mut f = File::open("misc/allsky_panstarrs.fits").unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();

        let mut reader = Cursor::new(&buf[..]);
        Fits::from_reader(&mut reader).unwrap();
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
    }*/
}
