use std::convert::TryFrom;
use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Once;

use image::error::{DecodingError, ImageFormatHint};
use image::hooks::{register_decoding_hook, register_format_detection_hook, GenericReader};
use image::{ColorType, ImageDecoder, ImageError, ImageResult};

use crate::fits::HDU as FitsHDU;
use crate::hdu::data::image::Pixels;
use crate::hdu::header::extension::image::Image as FitsImage;
use crate::hdu::header::Bitpix;
use crate::{Fits, HDU};
use serde::de::IntoDeserializer;
use serde::Deserialize;
use std::ops::DivAssign;

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

fn to_image_error(err: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> ImageError {
    ImageError::Decoding(DecodingError::new(
        ImageFormatHint::Name("fits".into()),
        std::io::Error::other(err),
    ))
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
            match fits.next().transpose().map_err(to_image_error)? {
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

#[derive(Deserialize, Clone, Copy, PartialEq)]
#[serde(default, rename_all = "UPPERCASE")]
struct Scale {
    bzero: f64,
    bscale: f64,
}

impl Default for Scale {
    fn default() -> Self {
        Self {
            bzero: 0.0,
            bscale: 1.0,
        }
    }
}

impl DivAssign<f64> for Scale {
    fn div_assign(&mut self, rhs: f64) {
        self.bzero /= rhs;
        self.bscale /= rhs;
    }
}

impl Scale {
    fn apply(self, value: f64) -> f64 {
        value.mul_add(self.bscale, self.bzero)
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
        let header = self.hdu.get_header();
        let mut scale = Scale::deserialize(header.into_deserializer()).map_err(to_image_error)?;
        let image_data = self.fits.get_data(&self.hdu);
        // The general idea of these conversions to an image suitable for viewing:
        // - ignore negative values
        // - assume images are already normalized to their pixel format max value
        // - adapt to the least lossy image-rs color type for given pixel format
        match image_data.pixels() {
            Pixels::U8(iter) => {
                let needs_scale = scale != Scale::default();

                for (dst, mut src) in buf.iter_mut().zip(iter) {
                    if needs_scale {
                        src = scale.apply(f64::from(src)).round() as u8;
                    }
                    *dst = src;
                }
            }
            Pixels::I16(iter) => {
                let needs_scale = scale != Scale::default();

                for (dst, src) in buf.as_chunks_mut::<2>().0.iter_mut().zip(iter) {
                    *dst = if needs_scale {
                        scale.apply(f64::from(src)).round() as u16
                    } else {
                        u16::try_from(src).unwrap_or(0)
                    }
                    .to_le_bytes();
                }
            }
            // For larger depths there is no matching image-rs color type, so we convert to Rgb32F and write the same value to all 3 channels.
            Pixels::I32(iter) => {
                // this is ugly, but image-rs doesn't have 32-bit integer format, so instead we scale down to f32 0-1 range
                scale /= f64::from(i32::MAX);

                for (chunk, src) in buf.as_chunks_mut::<12>().0.iter_mut().zip(iter) {
                    let src = (scale.apply(f64::from(src)) as f32).to_le_bytes();

                    // splat the same value to RGB since there is no floating-point luma ColorType in image-rs
                    for dst in chunk.as_chunks_mut::<4>().0 {
                        *dst = src;
                    }
                }
            }
            Pixels::I64(iter) => {
                // same as above, but for 64-bit integers
                scale /= i64::MAX as f64;

                for (chunk, src) in buf.as_chunks_mut::<12>().0.iter_mut().zip(iter) {
                    let src = (scale.apply(src as f64) as f32).to_le_bytes();

                    // splat the same value to RGB since there is no floating-point luma ColorType in image-rs
                    for dst in chunk.as_chunks_mut::<4>().0 {
                        *dst = src;
                    }
                }
            }
            // BZERO and BSCALE are not recommended for floating-point data in spec, so ignore it.
            Pixels::F32(iter) => {
                for (chunk, src) in buf.as_chunks_mut::<12>().0.iter_mut().zip(iter) {
                    // splat the same value to RGB since there is no floating-point luma ColorType in image-rs
                    for dst in chunk.as_chunks_mut::<4>().0 {
                        *dst = src.to_le_bytes();
                    }
                }
            }
            Pixels::F64(iter) => {
                for (chunk, src) in buf.as_chunks_mut::<12>().0.iter_mut().zip(iter) {
                    // splat the same value to RGB since there is no floating-point luma ColorType in image-rs
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
