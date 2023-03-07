use crate::hdu::header::extension::image::Image;
use crate::hdu::header::extension::asciitable::AsciiTable;
use crate::hdu::header::extension::bintable::BinTable;

use crate::hdu::HDU;
use crate::hdu::primary::consume_next_card;
use crate::hdu::header::extension::parse_xtension_card;
use super::header::extension::XtensionType;
use crate::error::Error;

use crate::hdu::DataBufRead;

#[derive(Debug)]
pub enum XtensionHDU<'a, R>
where
    R: DataBufRead<'a, Image> +
       DataBufRead<'a, BinTable> +
       DataBufRead<'a, AsciiTable> +
       'a
{
    Image(HDU<'a, R, Image>),
    AsciiTable(HDU<'a, R, AsciiTable>),
    BinTable(HDU<'a, R, BinTable>),
}

impl<'a, R> XtensionHDU<'a, R>
where
R: DataBufRead<'a, Image> +
   DataBufRead<'a, BinTable> +
   DataBufRead<'a, AsciiTable> +
   'a
{
    pub fn new(reader: &'a mut R) -> Result<Self, Error> {
        let mut num_bytes_read = 0;
        let mut card_80_bytes_buf = [0; 80];

        // XTENSION
        consume_next_card(reader, &mut card_80_bytes_buf, &mut num_bytes_read)?;
        let xtension_type = parse_xtension_card(&card_80_bytes_buf)?;

        let hdu = match xtension_type {
            XtensionType::Image => XtensionHDU::Image(HDU::<'a, R, Image>::new(reader, &mut num_bytes_read, &mut card_80_bytes_buf)?),
            XtensionType::BinTable => XtensionHDU::BinTable(HDU::<'a, R, BinTable>::new(reader, &mut num_bytes_read, &mut card_80_bytes_buf)?),
            XtensionType::AsciiTable => XtensionHDU::AsciiTable(HDU::<'a, R, AsciiTable>::new(reader, &mut num_bytes_read, &mut card_80_bytes_buf)?),
        };

        Ok(hdu)
    }

    fn consume(self) -> Result<Option<&'a mut R>, Error> {
        match self {
            XtensionHDU::Image(hdu) => hdu.consume(),
            XtensionHDU::AsciiTable(hdu) => hdu.consume(),
            XtensionHDU::BinTable(hdu) => hdu.consume(),
        }
    }

    pub fn next(self) -> Result<Option<Self>, Error> {
        if let Some(reader) = self.consume()? {
            let hdu = Self::new(reader)?;

            Ok(Some(hdu))
        } else {
            Ok(None)
        }
    }
}
