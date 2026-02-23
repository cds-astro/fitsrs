use std::convert::TryFrom;
use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::ops::DivAssign;
use std::sync::Once;

use image::error::{DecodingError, ImageFormatHint};
use image::hooks::{register_decoding_hook, register_format_detection_hook, GenericReader};
use image::metadata::Orientation;
use image::{ColorType, ImageDecoder, ImageError, ImageResult};

use serde::de::IntoDeserializer;
use serde::Deserialize;

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
    width: u32,
    height: u32,
    is_rgb: bool,
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
        let &[width, height, ref extra_axes @ ..] = xtension.get_naxis() else {
            return Err(to_image_error("primary HDU has fewer than 2 axes"));
        };

        let is_rgb = match extra_axes {
            [] | [1] => false,
            [3] => true,
            _ => {
                return Err(to_image_error(
                    "incompatible image axes (expected 2 or 3 with NAXIS3=3 for RGB)",
                ))
            }
        };

        Ok(Self {
            width: u32::try_from(width)
                .map_err(|_| to_image_error("image width exceeds u32::MAX"))?,
            height: u32::try_from(height)
                .map_err(|_| to_image_error("image height exceeds u32::MAX"))?,
            is_rgb,
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

const RGB_CHANNELS: usize = 3;

/// Write `iter` of `P`-byte values into `buf` (`P*3` bytes per pixel), de-planing from
/// FITS sequential-plane order into interleaved RGB order.
fn write_rgb<const P: usize>(buf: &mut [u8], mut iter: impl Iterator<Item = [u8; P]>) {
    let pixels = buf.as_chunks_mut::<P>().0.as_chunks_mut::<RGB_CHANNELS>().0;

    for channel in 0..RGB_CHANNELS {
        for (pixel, src) in pixels.iter_mut().zip(&mut iter) {
            pixel[channel] = src;
        }
    }
}

/// Write `iter` of `P`-byte values sequentially into `buf` (`P` bytes per pixel).
fn write_luma<const P: usize>(buf: &mut [u8], iter: impl Iterator<Item = [u8; P]>) {
    for (dst, src) in buf.as_chunks_mut::<P>().0.iter_mut().zip(iter) {
        *dst = src;
    }
}

/// Write `iter` of `P`-byte values into `buf` (`P*3` bytes per pixel), splatting each
/// value across all three channels (grayscale-as-RGB).
fn write_splat<const P: usize>(buf: &mut [u8], iter: impl Iterator<Item = [u8; P]>) {
    let pixels = buf.as_chunks_mut::<P>().0.as_chunks_mut::<RGB_CHANNELS>().0;

    for (pixel, bytes) in pixels.iter_mut().zip(iter) {
        *pixel = [bytes; RGB_CHANNELS];
    }
}

impl<'a> ImageDecoder for FitsDecoder<'a> {
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn orientation(&mut self) -> ImageResult<Orientation> {
        // FITS uses bottom-left coordinate system.
        // image-rs, like most modern formats, expects top-left one.
        // For correct display, provide a flip-vertical orientation hint.
        Ok(Orientation::FlipVertical)
    }

    fn color_type(&self) -> ColorType {
        match self.hdu.get_header().get_xtension().get_bitpix() {
            Bitpix::U8 => {
                if self.is_rgb {
                    ColorType::Rgb8
                } else {
                    ColorType::L8
                }
            }
            Bitpix::I16 => {
                if self.is_rgb {
                    ColorType::Rgb16
                } else {
                    ColorType::L16
                }
            }
            Bitpix::F32 | Bitpix::I32 | Bitpix::I64 | Bitpix::F64 => ColorType::Rgb32F,
        }
    }

    fn read_image(mut self, buf: &mut [u8]) -> ImageResult<()> {
        let header = self.hdu.get_header();
        let mut scale = Scale::deserialize(header.into_deserializer()).map_err(to_image_error)?;
        let is_rgb = self.is_rgb;
        let image_data = self.fits.get_data(&self.hdu);
        // The general idea of these conversions to an image suitable for viewing:
        // - ignore negative values
        // - assume images are already normalized to their pixel format max value
        // - adapt to the least lossy image-rs color type for given pixel format
        match image_data.pixels() {
            Pixels::U8(iter) => {
                let needs_scale = scale != Scale::default();
                let mapped = iter.map(|src| {
                    [if needs_scale {
                        scale.apply(f64::from(src)).round() as u8
                    } else {
                        src
                    }]
                });
                if is_rgb {
                    write_rgb(buf, mapped);
                } else {
                    write_luma(buf, mapped);
                }
            }
            Pixels::I16(iter) => {
                let needs_scale = scale != Scale::default();
                let mapped = iter.map(|src| {
                    if needs_scale {
                        scale.apply(f64::from(src)).round() as u16
                    } else {
                        u16::try_from(src).unwrap_or(0)
                    }
                    .to_le_bytes()
                });
                if is_rgb {
                    write_rgb(buf, mapped);
                } else {
                    write_luma(buf, mapped);
                }
            }
            // For larger depths there is no matching image-rs color type, so we convert to Rgb32F and write the same value to all 3 channels.
            Pixels::I32(iter) => {
                // this is ugly, but image-rs doesn't have 32-bit integer format, so instead we scale down to f32 0-1 range
                scale /= f64::from(i32::MAX);
                let mapped = iter.map(|src| (scale.apply(f64::from(src)) as f32).to_le_bytes());
                if is_rgb {
                    write_rgb(buf, mapped);
                } else {
                    write_splat(buf, mapped);
                }
            }
            Pixels::I64(iter) => {
                // same as above, but for 64-bit integers
                scale /= i64::MAX as f64;
                let mapped = iter.map(|src| (scale.apply(src as f64) as f32).to_le_bytes());
                if is_rgb {
                    write_rgb(buf, mapped);
                } else {
                    write_splat(buf, mapped);
                }
            }
            // BZERO and BSCALE are not recommended for floating-point data in spec, so ignore it.
            Pixels::F32(iter) => {
                let mapped = iter.map(|src| src.to_le_bytes());
                if is_rgb {
                    write_rgb(buf, mapped);
                } else {
                    write_splat(buf, mapped);
                }
            }
            Pixels::F64(iter) => {
                let mapped = iter.map(|src| (src as f32).to_le_bytes());
                if is_rgb {
                    write_rgb(buf, mapped);
                } else {
                    write_splat(buf, mapped);
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
