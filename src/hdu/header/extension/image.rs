use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;

use crate::card::Value;
use crate::error::Error;
use crate::hdu::header::check_for_bitpix;
use crate::hdu::header::check_for_naxis;
use crate::hdu::header::Bitpix;

use crate::hdu::header::Xtension;

#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Image {
    // A number of bit that each pixel has
    bitpix: Bitpix,
    // The number of axis
    naxis: usize,
    // The size of each axis
    naxisn: Vec<u64>,
}

impl Image {
    /// Get the number of axis given by the "NAXIS" card
    pub fn get_naxis(&self) -> usize {
        self.naxis
    }

    /// Get the size of an axis given by the "NAXISX" card
    pub fn get_naxisn(&self, idx: usize) -> Option<&u64> {
        // NAXIS indexes begins at 1 instead of 0
        self.naxisn.get(idx - 1)
    }

    pub fn get_naxisn_all(&self) -> &[u64] {
        self.naxisn.as_slice()
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

    fn parse(values: &HashMap<String, Value>) -> Result<Self, Error> {
        // BITPIX
        let bitpix = check_for_bitpix(values)?;
        // NAXIS
        let naxis = check_for_naxis(values)?;
        // The size of each NAXIS
        let naxisn = (1..=naxis)
            .map(|naxis_i| {
                let naxis = format!("NAXIS{naxis_i}");
                if let Some(Value::Integer { value, .. }) = values.get(&naxis) {
                    Ok(*value as u64)
                } else {
                    Err(Error::FailFindingKeyword(naxis))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Image {
            bitpix,
            naxis: naxis as usize,
            naxisn,
        })
    }
}
