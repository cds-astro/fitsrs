use async_trait::async_trait;
use serde::Serialize;

use crate::error::Error;
use crate::hdu::header::check_for_bitpix;
use crate::hdu::header::check_for_naxis;
use crate::hdu::header::Bitpix;

use crate::hdu::header::ValueMap;
use crate::hdu::header::Xtension;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Image {
    // A number of bit that each pixel has
    bitpix: Bitpix,
    // The size of each axis
    naxisn: Box<[u64]>,
}

impl Image {
    /// Get the sizes of axis given by the "NAXIS" cards
    pub fn get_naxis(&self) -> &[u64] {
        &self.naxisn
    }

    /// Get the bitpix value given by the "BITPIX" card
    pub fn get_bitpix(&self) -> Bitpix {
        self.bitpix
    }

    /// Get total number of pixels in the image
    pub fn get_num_pixels(&self) -> u64 {
        if self.naxisn.is_empty() {
            return 0;
        }
        self.naxisn.iter().product()
    }
}

#[async_trait(?Send)]
impl Xtension for Image {
    fn get_num_bytes_data_block(&self) -> u64 {
        self.bitpix.byte_size() as u64 * self.get_num_pixels()
    }

    fn parse(values: &ValueMap) -> Result<Self, Error> {
        // BITPIX
        let bitpix = check_for_bitpix(values)?;
        // NAXIS
        let naxis = check_for_naxis(values)?;
        // The size of each NAXIS
        let naxisn = (1..=naxis)
            .map(|naxis_i| {
                values
                    .get_parsed(&format!("NAXIS{naxis_i}"))
                    .map(|value: i64| value as _)
            })
            .collect::<Result<_, _>>()?;

        Ok(Image { bitpix, naxisn })
    }
}
