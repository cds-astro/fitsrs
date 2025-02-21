use wcs::WCSParams;
use crate::hdu::header::extension::image::Image;
use crate::hdu::header::Header;
use std::convert::TryFrom;
use crate::error::Error;
use crate::card::CardValue;
use std::str::FromStr;

pub type ImgXY = wcs::ImgXY;
pub type LonLat = wcs::LonLat;

use wcs::WCS;
use crate::fits::HDU;
use std::convert::TryInto;
impl HDU<Image> {
    /// Try to look for a WCS in the image header and return a [WCS](https://crates.io/crates/wcs) object
    pub fn wcs(&self) -> Result<WCS, Error> {
        self.get_header().try_into()
    }
}

fn parse_optional_card_with_type<T: CardValue + FromStr>(header: &Header<Image>, key: &'static str) -> Result<Option<T>, Error> {
    match  header.get_parsed::<T>(key).transpose() {
        Ok(v) => Ok(v),
        _ => {
            let str = header.get_parsed::<String>(key).transpose()
                .unwrap_or(None);

            Ok(if let Some(ss) = str {
                ss.trim().parse::<T>()
                    .map(|v| Some(v))
                    .unwrap_or(None)
            } else {
                // card not found but it is ok as it is not mandatory
                None
            })
        }
    }
}

fn parse_mandatory_card_with_type<T: CardValue>(header: &Header<Image>, key: &'static str) -> Result<T, Error> {
    match header.get_parsed::<T>(key) {
        // No parsing error and found
        Some(Ok(v)) => {
            Ok(v)
        },
        // No error but not found, we return an error
        None => Err(Error::WCS),
        // Return the parsing error
        Some(Err(e)) => {
            Err(e.into())
        }
    }
}

impl<'a> TryFrom<&'a Header<Image>> for WCS {
    type Error = Error;

    fn try_from(h: &'a Header<Image>) -> Result<Self, Self::Error> {
        let params = WCSParams {
            ctype1: parse_mandatory_card_with_type::<String>(h, "CTYPE1")?,
            naxis: parse_mandatory_card_with_type::<i64>(h, "NAXIS")?,

            naxis1: parse_optional_card_with_type::<i64>(h, "NAXIS1")?,
            naxis2: parse_optional_card_with_type::<i64>(h, "NAXIS2")?,
            ctype2: parse_optional_card_with_type::<String>(h, "CTYPE2")?,
            ctype3: parse_optional_card_with_type::<String>(h, "CTYPE3")?,
            a_order: parse_optional_card_with_type::<i64>(h, "A_ORDER")?,
            b_order: parse_optional_card_with_type::<i64>(h, "B_ORDER")?,
            ap_order: parse_optional_card_with_type::<i64>(h, "AP_ORDER")?,
            bp_order: parse_optional_card_with_type::<i64>(h, "BP_ORDER")?,
            crpix1: parse_optional_card_with_type::<f64>(h, "CRPIX1")?,
            crpix2: parse_optional_card_with_type::<f64>(h, "CRPIX2")?,
            crpix3: parse_optional_card_with_type::<f64>(h, "CRPIX3")?,
            crval1: parse_optional_card_with_type::<f64>(h, "CRVAL1")?,
            crval2: parse_optional_card_with_type::<f64>(h, "CRVAL2")?,
            crval3: parse_optional_card_with_type::<f64>(h, "CRVAL3")?,
            crota1: parse_optional_card_with_type::<f64>(h, "CROTA1")?,
            crota2: parse_optional_card_with_type::<f64>(h, "CROTA2")?,
            crota3: parse_optional_card_with_type::<f64>(h, "CROTA3")?,
            cdelt1: parse_optional_card_with_type::<f64>(h, "CDELT1")?,
            cdelt2: parse_optional_card_with_type::<f64>(h, "CDELT2")?,
            cdelt3: parse_optional_card_with_type::<f64>(h, "CDELT3")?,
            naxis3: parse_optional_card_with_type::<i64>(h, "NAXIS3")?,
            naxis4: parse_optional_card_with_type::<i64>(h, "NAXIS4")?,
            lonpole: parse_optional_card_with_type::<f64>(h, "LONPOLE")?,
            latpole: parse_optional_card_with_type::<f64>(h, "LATPOLE")?,
            equinox: parse_optional_card_with_type::<f64>(h, "EQUINOX")?,
            epoch: parse_optional_card_with_type::<f64>(h, "EPOCH")?,
            radesys: parse_optional_card_with_type::<String>(h, "RADESYS")?,
            pv1_0: parse_optional_card_with_type::<f64>(h, "PV1_0")?,
            pv1_1: parse_optional_card_with_type::<f64>(h, "PV1_1")?,
            pv1_2: parse_optional_card_with_type::<f64>(h, "PV1_2")?,
            pv2_0: parse_optional_card_with_type::<f64>(h, "PV2_0")?,
            pv2_1: parse_optional_card_with_type::<f64>(h, "PV2_1")?,
            pv2_2: parse_optional_card_with_type::<f64>(h, "PV2_2")?,
            pv2_3: parse_optional_card_with_type::<f64>(h, "PV2_3")?,
            pv2_4: parse_optional_card_with_type::<f64>(h, "PV2_4")?,
            pv2_5: parse_optional_card_with_type::<f64>(h, "PV2_5")?,
            pv2_6: parse_optional_card_with_type::<f64>(h, "PV2_6")?,
            pv2_7: parse_optional_card_with_type::<f64>(h, "PV2_7")?,
            pv2_8: parse_optional_card_with_type::<f64>(h, "PV2_8")?,
            pv2_9: parse_optional_card_with_type::<f64>(h, "PV2_9")?,
            pv2_10: parse_optional_card_with_type::<f64>(h, "PV2_10")?,
            pv2_11: parse_optional_card_with_type::<f64>(h, "PV2_11")?,
            pv2_12: parse_optional_card_with_type::<f64>(h, "PV2_12")?,
            pv2_13: parse_optional_card_with_type::<f64>(h, "PV2_13")?,
            pv2_14: parse_optional_card_with_type::<f64>(h, "PV2_14")?,
            pv2_15: parse_optional_card_with_type::<f64>(h, "PV2_15")?,
            pv2_16: parse_optional_card_with_type::<f64>(h, "PV2_16")?,
            pv2_17: parse_optional_card_with_type::<f64>(h, "PV2_17")?,
            pv2_18: parse_optional_card_with_type::<f64>(h, "PV2_18")?,
            pv2_19: parse_optional_card_with_type::<f64>(h, "PV2_19")?,
            pv2_20: parse_optional_card_with_type::<f64>(h, "PV2_20")?,
            cd1_1: parse_optional_card_with_type::<f64>(h, "CD1_1")?,
            cd1_2: parse_optional_card_with_type::<f64>(h, "CD1_2")?,
            cd1_3: parse_optional_card_with_type::<f64>(h, "CD1_3")?,
            cd2_1: parse_optional_card_with_type::<f64>(h, "CD2_1")?,
            cd2_2: parse_optional_card_with_type::<f64>(h, "CD2_2")?,
            cd2_3: parse_optional_card_with_type::<f64>(h, "CD2_3")?,
            cd3_1: parse_optional_card_with_type::<f64>(h, "CD3_1")?,
            cd3_2: parse_optional_card_with_type::<f64>(h, "CD3_2")?,
            cd3_3: parse_optional_card_with_type::<f64>(h, "CD3_3")?,
            pc1_1: parse_optional_card_with_type::<f64>(h, "PC1_1")?,
            pc1_2: parse_optional_card_with_type::<f64>(h, "PC1_2")?,
            pc1_3: parse_optional_card_with_type::<f64>(h, "PC1_3")?,
            pc2_1: parse_optional_card_with_type::<f64>(h, "PC2_1")?,
            pc2_2: parse_optional_card_with_type::<f64>(h, "PC2_2")?,
            pc2_3: parse_optional_card_with_type::<f64>(h, "PC2_3")?,
            pc3_1: parse_optional_card_with_type::<f64>(h, "PC3_1")?,
            pc3_2: parse_optional_card_with_type::<f64>(h, "PC3_2")?,
            pc3_3: parse_optional_card_with_type::<f64>(h, "PC3_3")?,
            a_0_0: parse_optional_card_with_type::<f64>(h, "A_0_0")?,
            a_1_0: parse_optional_card_with_type::<f64>(h, "A_1_0")?,
            a_2_0: parse_optional_card_with_type::<f64>(h, "A_2_0")?,
            a_3_0: parse_optional_card_with_type::<f64>(h, "A_3_0")?,
            a_4_0: parse_optional_card_with_type::<f64>(h, "A_4_0")?,
            a_5_0: parse_optional_card_with_type::<f64>(h, "A_5_0")?,
            a_6_0: parse_optional_card_with_type::<f64>(h, "A_6_0")?,
            a_0_1: parse_optional_card_with_type::<f64>(h, "A_0_1")?,
            a_1_1: parse_optional_card_with_type::<f64>(h, "A_1_1")?,
            a_2_1: parse_optional_card_with_type::<f64>(h, "A_2_1")?,
            a_3_1: parse_optional_card_with_type::<f64>(h, "A_3_1")?,
            a_4_1: parse_optional_card_with_type::<f64>(h, "A_4_1")?,
            a_5_1: parse_optional_card_with_type::<f64>(h, "A_5_1")?,
            a_0_2: parse_optional_card_with_type::<f64>(h, "A_0_2")?,
            a_1_2: parse_optional_card_with_type::<f64>(h, "A_1_2")?,
            a_2_2: parse_optional_card_with_type::<f64>(h, "A_2_2")?,
            a_3_2: parse_optional_card_with_type::<f64>(h, "A_3_2")?,
            a_4_2: parse_optional_card_with_type::<f64>(h, "A_4_2")?,
            a_0_3: parse_optional_card_with_type::<f64>(h, "A_0_3")?,
            a_1_3: parse_optional_card_with_type::<f64>(h, "A_1_3")?,
            a_2_3: parse_optional_card_with_type::<f64>(h, "A_2_3")?,
            a_3_3: parse_optional_card_with_type::<f64>(h, "A_3_3")?,
            a_0_4: parse_optional_card_with_type::<f64>(h, "A_0_4")?,
            a_1_4: parse_optional_card_with_type::<f64>(h, "A_1_4")?,
            a_2_4: parse_optional_card_with_type::<f64>(h, "A_2_4")?,
            a_0_5: parse_optional_card_with_type::<f64>(h, "A_0_5")?,
            a_1_5: parse_optional_card_with_type::<f64>(h, "A_1_5")?,
            a_0_6: parse_optional_card_with_type::<f64>(h, "A_0_6")?,
            ap_0_0: parse_optional_card_with_type::<f64>(h, "AP_0_0")?,
            ap_1_0: parse_optional_card_with_type::<f64>(h, "AP_1_0")?,
            ap_2_0: parse_optional_card_with_type::<f64>(h, "AP_2_0")?,
            ap_3_0: parse_optional_card_with_type::<f64>(h, "AP_3_0")?,
            ap_4_0: parse_optional_card_with_type::<f64>(h, "AP_4_0")?,
            ap_5_0: parse_optional_card_with_type::<f64>(h, "AP_5_0")?,
            ap_6_0: parse_optional_card_with_type::<f64>(h, "AP_6_0")?,
            ap_0_1: parse_optional_card_with_type::<f64>(h, "AP_0_1")?,
            ap_1_1: parse_optional_card_with_type::<f64>(h, "AP_1_1")?,
            ap_2_1: parse_optional_card_with_type::<f64>(h, "AP_2_1")?,
            ap_3_1: parse_optional_card_with_type::<f64>(h, "AP_3_1")?,
            ap_4_1: parse_optional_card_with_type::<f64>(h, "AP_4_1")?,
            ap_5_1: parse_optional_card_with_type::<f64>(h, "AP_5_1")?,
            ap_0_2: parse_optional_card_with_type::<f64>(h, "AP_0_2")?,
            ap_1_2: parse_optional_card_with_type::<f64>(h, "AP_1_2")?,
            ap_2_2: parse_optional_card_with_type::<f64>(h, "AP_2_2")?,
            ap_3_2: parse_optional_card_with_type::<f64>(h, "AP_3_2")?,
            ap_4_2: parse_optional_card_with_type::<f64>(h, "AP_4_2")?,
            ap_0_3: parse_optional_card_with_type::<f64>(h, "AP_0_3")?,
            ap_1_3: parse_optional_card_with_type::<f64>(h, "AP_1_3")?,
            ap_2_3: parse_optional_card_with_type::<f64>(h, "AP_2_3")?,
            ap_3_3: parse_optional_card_with_type::<f64>(h, "AP_3_3")?,
            ap_0_4: parse_optional_card_with_type::<f64>(h, "AP_0_4")?,
            ap_1_4: parse_optional_card_with_type::<f64>(h, "AP_1_4")?,
            ap_2_4: parse_optional_card_with_type::<f64>(h, "AP_2_4")?,
            ap_0_5: parse_optional_card_with_type::<f64>(h, "AP_0_5")?,
            ap_1_5: parse_optional_card_with_type::<f64>(h, "AP_1_5")?,
            ap_0_6: parse_optional_card_with_type::<f64>(h, "AP_0_6")?,
            b_0_0: parse_optional_card_with_type::<f64>(h, "B_0_0")?,
            b_1_0: parse_optional_card_with_type::<f64>(h, "B_1_0")?,
            b_2_0: parse_optional_card_with_type::<f64>(h, "B_2_0")?,
            b_3_0: parse_optional_card_with_type::<f64>(h, "B_3_0")?,
            b_4_0: parse_optional_card_with_type::<f64>(h, "B_4_0")?,
            b_5_0: parse_optional_card_with_type::<f64>(h, "B_5_0")?,
            b_6_0: parse_optional_card_with_type::<f64>(h, "B_6_0")?,
            b_0_1: parse_optional_card_with_type::<f64>(h, "B_0_1")?,
            b_1_1: parse_optional_card_with_type::<f64>(h, "B_1_1")?,
            b_2_1: parse_optional_card_with_type::<f64>(h, "B_2_1")?,
            b_3_1: parse_optional_card_with_type::<f64>(h, "B_3_1")?,
            b_4_1: parse_optional_card_with_type::<f64>(h, "B_4_1")?,
            b_5_1: parse_optional_card_with_type::<f64>(h, "B_5_1")?,
            b_0_2: parse_optional_card_with_type::<f64>(h, "B_0_2")?,
            b_1_2: parse_optional_card_with_type::<f64>(h, "B_1_2")?,
            b_2_2: parse_optional_card_with_type::<f64>(h, "B_2_2")?,
            b_3_2: parse_optional_card_with_type::<f64>(h, "B_3_2")?,
            b_4_2: parse_optional_card_with_type::<f64>(h, "B_4_2")?,
            b_0_3: parse_optional_card_with_type::<f64>(h, "B_0_3")?,
            b_1_3: parse_optional_card_with_type::<f64>(h, "B_1_3")?,
            b_2_3: parse_optional_card_with_type::<f64>(h, "B_2_3")?,
            b_3_3: parse_optional_card_with_type::<f64>(h, "B_3_3")?,
            b_0_4: parse_optional_card_with_type::<f64>(h, "B_0_4")?,
            b_1_4: parse_optional_card_with_type::<f64>(h, "B_1_4")?,
            b_2_4: parse_optional_card_with_type::<f64>(h, "B_2_4")?,
            b_0_5: parse_optional_card_with_type::<f64>(h, "B_0_5")?,
            b_1_5: parse_optional_card_with_type::<f64>(h, "B_1_5")?,
            b_0_6: parse_optional_card_with_type::<f64>(h, "B_0_6")?,
            bp_0_0: parse_optional_card_with_type::<f64>(h, "BP_0_0")?,
            bp_1_0: parse_optional_card_with_type::<f64>(h, "BP_1_0")?,
            bp_2_0: parse_optional_card_with_type::<f64>(h, "BP_2_0")?,
            bp_3_0: parse_optional_card_with_type::<f64>(h, "BP_3_0")?,
            bp_4_0: parse_optional_card_with_type::<f64>(h, "BP_4_0")?,
            bp_5_0: parse_optional_card_with_type::<f64>(h, "BP_5_0")?,
            bp_6_0: parse_optional_card_with_type::<f64>(h, "BP_6_0")?,
            bp_0_1: parse_optional_card_with_type::<f64>(h, "BP_0_1")?,
            bp_1_1: parse_optional_card_with_type::<f64>(h, "BP_1_1")?,
            bp_2_1: parse_optional_card_with_type::<f64>(h, "BP_2_1")?,
            bp_3_1: parse_optional_card_with_type::<f64>(h, "BP_3_1")?,
            bp_4_1: parse_optional_card_with_type::<f64>(h, "BP_4_1")?,
            bp_5_1: parse_optional_card_with_type::<f64>(h, "BP_5_1")?,
            bp_0_2: parse_optional_card_with_type::<f64>(h, "BP_0_2")?,
            bp_1_2: parse_optional_card_with_type::<f64>(h, "BP_1_2")?,
            bp_2_2: parse_optional_card_with_type::<f64>(h, "BP_2_2")?,
            bp_3_2: parse_optional_card_with_type::<f64>(h, "BP_3_2")?,
            bp_4_2: parse_optional_card_with_type::<f64>(h, "BP_4_2")?,
            bp_0_3: parse_optional_card_with_type::<f64>(h, "BP_0_3")?,
            bp_1_3: parse_optional_card_with_type::<f64>(h, "BP_1_3")?,
            bp_2_3: parse_optional_card_with_type::<f64>(h, "BP_2_3")?,
            bp_3_3: parse_optional_card_with_type::<f64>(h, "BP_3_3")?,
            bp_0_4: parse_optional_card_with_type::<f64>(h, "BP_0_4")?,
            bp_1_4: parse_optional_card_with_type::<f64>(h, "BP_1_4")?,
            bp_2_4: parse_optional_card_with_type::<f64>(h, "BP_2_4")?,
            bp_0_5: parse_optional_card_with_type::<f64>(h, "BP_0_5")?,
            bp_1_5: parse_optional_card_with_type::<f64>(h, "BP_1_5")?,
            bp_0_6: parse_optional_card_with_type::<f64>(h, "BP_0_6")?,
        };

        WCS::new(&params).map_err(|e| e.into())
    }
}
