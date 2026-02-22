use std::convert::TryFrom;
use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Once;

use image::error::{DecodingError, ImageFormatHint};
use image::hooks::{register_decoding_hook, register_format_detection_hook, GenericReader};
use image::{ColorType, ImageDecoder, ImageError, ImageResult};

use crate::error::Error as FitsError;
use crate::fits::HDU as FitsHDU;
use crate::hdu::data::image::Pixels;
use crate::hdu::header::extension::image::Image as FitsImage;
use crate::hdu::header::Bitpix;
use crate::{Fits, HDU};

/// Newtype around `GenericReader<'a>` that adds a trivial `Debug` impl,
/// satisfying `Fits<R>`'s `R: Debug` bound.
struct FitsReader<'a>(GenericReader<'a>);

impl fmt::Debug for FitsReader<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FitsReader").finish_non_exhaustive()
    }
}

impl Read for FitsReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for FitsReader<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

fn to_image_error(err: impl Into<String>) -> ImageError {
    let msg = err.into();
    ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("fits".into()),
        std::io::Error::other(msg),
    ))
}

fn map_fits_error(err: FitsError) -> ImageError {
    to_image_error(err.to_string())
}

/// Registers the FITS decoder with the `image` crate so that calls such as
/// `ImageReader::open("image.fits")?.decode()?` work with FITS files.
///
/// Should be called once before the first use, subsequent.
pub fn register_fits_decoding_hook() {
    static REGISTER_FITS_HOOK: Once = Once::new();

    REGISTER_FITS_HOOK.call_once(|| {
        for ext in ["fits", "fit", "fts", "fz"] {
            register_decoding_hook(ext.into(), Box::new(|r| Ok(Box::new(FitsDecoder::new(r)?))));
        }

        register_format_detection_hook("fits".into(), b"SIMPLE  =", None);
    });
}

pub struct FitsDecoder<'a> {
    dimensions: (u32, u32),
    color_type: ColorType,
    fits: Fits<FitsReader<'a>>,
    hdu: FitsHDU<FitsImage>,
}

impl<'a> FitsDecoder<'a> {
    pub fn new(reader: GenericReader<'a>) -> ImageResult<Self> {
        let mut fits = Fits::from_reader(FitsReader(reader));

        // The primary HDU is always an image, but it may have no data,
        // in which case the actual image is in the first XImage extension.
        let hdu = loop {
            match fits.next().transpose().map_err(map_fits_error)? {
                Some(HDU::Primary(hdu)) | Some(HDU::XImage(hdu))
                    if hdu.get_data_unit_byte_size() != 0 =>
                {
                    break hdu
                }
                Some(_) => {}
                None => return Err(to_image_error("no 2D image HDU found in FITS file")),
            }
        };

        let xtension: &FitsImage = hdu.get_header().get_xtension();
        let axes = xtension.get_naxis();

        if axes.len() < 2 {
            return Err(to_image_error("primary HDU has fewer than 2 axes"));
        }

        let color_type = match xtension.get_bitpix() {
            Bitpix::U8 => ColorType::L8,
            Bitpix::I16 => ColorType::L16,
            Bitpix::F32 | Bitpix::I32 | Bitpix::I64 | Bitpix::F64 => ColorType::Rgb32F,
        };

        let width = u32::try_from(axes[0])
            .ok()
            .filter(|&w| w > 0)
            .ok_or_else(|| to_image_error("NAXIS1 is zero or out of range"))?;
        let height = u32::try_from(axes[1])
            .ok()
            .filter(|&h| h > 0)
            .ok_or_else(|| to_image_error("NAXIS2 is zero or out of range"))?;

        Ok(Self {
            dimensions: (width, height),
            color_type,
            fits,
            hdu,
        })
    }
}

impl<'a> ImageDecoder for FitsDecoder<'a> {
    fn dimensions(&self) -> (u32, u32) {
        self.dimensions
    }

    fn color_type(&self) -> ColorType {
        self.color_type
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        // Read BZERO/BSCALE from the header at decode time; they default to 0 and 1
        // per the FITS standard when absent. The most important case is BZERO=32768
        // with BITPIX=16, which encodes unsigned u16 data as signed i16.
        let header = self.hdu.get_header();
        // BZERO/BSCALE default to 0 and 1 per the FITS standard when absent.
        // The FITS spec mandates these are always stored as real floating-point values.
        // get_parsed::<Option<f64>> returns Ok(None) when absent, Ok(Some(v)) when
        // present, and Err when present but not parseable as a float.
        let bzero = header
            .get_parsed::<Option<f64>>("BZERO")
            .map_err(map_fits_error)?
            .unwrap_or(0.0);
        let bscale = header
            .get_parsed::<Option<f64>>("BSCALE")
            .map_err(map_fits_error)?
            .unwrap_or(1.0);
        let scale =
            (bzero != 0.0 || bscale != 1.0).then_some(move |v: f64| v.mul_add(bscale, bzero));
        let image_data = self.fits.get_data(&self.hdu);
        match image_data.pixels() {
            Pixels::U8(iter) => {
                for (dst, mut src) in buf.iter_mut().zip(iter) {
                    if let Some(scale) = scale {
                        src = scale(src as f64).round() as u8;
                    }
                    *dst = src;
                }
            }
            Pixels::I16(iter) => {
                for (dst, mut src) in buf.as_chunks_mut::<2>().0.iter_mut().zip(iter) {
                    if let Some(scale) = scale {
                        src = scale(src as f64).round() as i16;
                    }
                    *dst = src.max(0).to_le_bytes();
                }
            }
            // For larger depths there is no matching image-rs color type, so we convert to Rgb32F and write the same value to all 3 channels.
            Pixels::I32(iter) => {
                for (chunk, src) in buf.as_chunks_mut::<12>().0.iter_mut().zip(iter) {
                    let src = match scale {
                        Some(scale) => scale(f64::from(src)) as f32,
                        None => src as f32,
                    }
                    .to_le_bytes();

                    for dst in chunk.as_chunks_mut::<4>().0 {
                        *dst = src;
                    }
                }
            }
            Pixels::I64(iter) => {
                for (chunk, src) in buf.as_chunks_mut::<12>().0.iter_mut().zip(iter) {
                    let src = match scale {
                        Some(scale) => scale(src as f64) as f32,
                        None => src as f32,
                    }
                    .to_le_bytes();

                    for dst in chunk.as_chunks_mut::<4>().0 {
                        *dst = src;
                    }
                }
            }
            // BZERO and BSCALE are not recommended for floating-point data in spec, so ignore it.
            Pixels::F32(iter) => {
                for (chunk, src) in buf.as_chunks_mut::<12>().0.iter_mut().zip(iter) {
                    for dst in chunk.as_chunks_mut::<4>().0 {
                        *dst = src.to_le_bytes();
                    }
                }
            }
            Pixels::F64(iter) => {
                for (chunk, src) in buf.as_chunks_mut::<12>().0.iter_mut().zip(iter) {
                    for dst in chunk.as_chunks_mut::<8>().0 {
                        *dst = src.to_le_bytes();
                    }
                }
            }
        };
        Ok(())
    }

    fn read_image_boxed(self: Box<Self>, buf: &mut [u8]) -> ImageResult<()> {
        (*self).read_image(buf)
    }

    fn icc_profile(&mut self) -> ImageResult<Option<Vec<u8>>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::register_fits_decoding_hook;
    use image::{ColorType, GenericImageView};
    use std::path::Path;
    use test_case::test_case;
    use ColorType::{Rgb32F, L16};

    #[test_case("fits.gsfc.nasa.gov/Astro_UIT", 512, 512, L16)]
    #[test_case("fits.gsfc.nasa.gov/EUVE", 512, 512, L16)]
    #[test_case("fits.gsfc.nasa.gov/HST_FGS", 89688, 7, Rgb32F)]
    #[test_case("fits.gsfc.nasa.gov/HST_FOC", 1024, 1024, Rgb32F)]
    #[test_case("fits.gsfc.nasa.gov/HST_FOS", 2064, 2, Rgb32F)]
    #[test_case("fits.gsfc.nasa.gov/HST_HRS", 2000, 4, Rgb32F)]
    #[test_case("fits.gsfc.nasa.gov/HST_NICMOS", 270, 263, Rgb32F)]
    #[test_case("fits.gsfc.nasa.gov/HST_WFPC_II", 200, 200, Rgb32F)]
    #[test_case("fits.gsfc.nasa.gov/HST_WFPC_II_bis", 100, 100, Rgb32F)]
    #[test_case("hipsgen/Npix8", 512, 512, L16)]
    #[test_case("hipsgen/Npix9", 512, 512, L16)]
    #[test_case("hipsgen/Npix132", 512, 512, L16)]
    #[test_case("hipsgen/Npix133", 512, 512, L16)]
    #[test_case("hipsgen/Npix134", 512, 512, L16)]
    #[test_case("hipsgen/Npix140", 512, 512, L16)]
    #[test_case("hipsgen/Npix208", 64, 64, Rgb32F)]
    #[test_case("hipsgen/Npix282", 64, 64, Rgb32F)]
    #[test_case("hipsgen/Npix4906", 512, 512, L16)]
    #[test_case("hipsgen/Npix691539", 512, 512, L16)]
    #[test_case("hips2fits/allsky_panstarrs", 1728, 1856, L16)]
    #[test_case("hips2fits/cutout-CDS_P_HST_PHAT_F475W", 3000, 3000, Rgb32F)]
    #[test_case("vizier/NVSSJ235137-362632r", 1024, 1024, Rgb32F)]
    #[test_case("vizier/VAR.358.R", 51, 51, Rgb32F)]
    #[test_case("misc/bonn", 300, 300, Rgb32F)]
    #[test_case("misc/skv1678175163788", 300, 300, Rgb32F)]
    #[test_case("misc/SN2923fxjA", 3194, 4784, Rgb32F)]
    fn decode_fits_via_image_reader(rel_path: &str, exp_w: u32, exp_h: u32, exp_color: ColorType) {
        register_fits_decoding_hook();

        let img = image::open(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("samples")
                .join(rel_path)
                .with_added_extension("fits"),
        )
        .expect("decoding failed");

        assert_eq!(img.dimensions(), (exp_w, exp_h), "dimensions mismatch");
        assert_eq!(img.color(), exp_color, "color type mismatch");
    }
}
