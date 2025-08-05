use crate::error::Error;
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::Header;
use std::convert::TryFrom;

pub type ImgXY = wcs::ImgXY;
pub type LonLat = wcs::LonLat;

use crate::fits::HDU;
use serde::de::IntoDeserializer;
use serde::Deserialize;
use std::convert::TryInto;
pub use wcs::{WCSParams, WCS};

impl HDU<Image> {
    /// Try to look for a WCS in the image header and return a [WCS](https://crates.io/crates/wcs) object
    pub fn wcs(&self) -> Result<WCS, Error> {
        self.get_header().try_into()
    }
}

impl<'a> TryFrom<&'a Header<Image>> for WCS {
    type Error = Error;

    fn try_from(h: &'a Header<Image>) -> Result<Self, Self::Error> {
        let params = WCSParams::deserialize(h.into_deserializer())?;
        WCS::new(&params).map_err(|e| e.into())
    }
}
