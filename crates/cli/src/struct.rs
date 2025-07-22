use std::{error::Error, fmt::Debug, fs::File, io::BufReader, path::PathBuf};

use clap::Args;

use fitsrs::{
    Fits, HDU, fits,
    hdu::header::{
        Xtension,
        extension::{asciitable::AsciiTable, bintable::BinTable, image::Image},
    },
};

#[derive(Debug, Clone, Args)]
pub struct Struct {
    /// Path of the input file.
    #[clap(value_name = "FILE")]
    pub input: PathBuf,
}

impl Struct {
    pub fn exec(self) -> Result<(), Box<dyn Error>> {
        let file = File::open(&self.input)?;
        let reader = BufReader::new(file);
        for (i, hdu) in Fits::from_reader(reader).enumerate() {
            hdu.map_err(|e| e.into())
                .and_then(|hdu| print_hdu_struct(i, hdu))?;
        }
        Ok(())
    }
}

fn print_hdu_struct(i: usize, hdu: HDU) -> Result<(), Box<dyn Error>> {
    println!("HDU[{}]:", i);
    match hdu {
        HDU::Primary(img) => print_primhdu_struct(img),
        HDU::XImage(img) => print_imghdu_struct(img),
        HDU::XBinaryTable(bintable) => print_bintablehdu_struct(bintable),
        HDU::XASCIITable(asciitable) => print_ascisstablehdu_struct(asciitable),
    }
}

fn print_primhdu_struct(hdu: fits::HDU<Image>) -> Result<(), Box<dyn Error>> {
    print_hdu_type("PRIMARY");
    print_img_header(hdu.get_header().get_xtension());
    print_data_struct(&hdu);
    Ok(())
}

fn print_imghdu_struct(hdu: fits::HDU<Image>) -> Result<(), Box<dyn Error>> {
    print_hdu_type("IMAGE");
    print_img_header(hdu.get_header().get_xtension());
    print_data_struct(&hdu);
    Ok(())
}

fn print_bintablehdu_struct(hdu: fits::HDU<BinTable>) -> Result<(), Box<dyn Error>> {
    print_hdu_type("BINTABLE");
    print_bintable_header(hdu.get_header().get_xtension());
    print_data_struct(&hdu);
    Ok(())
}

fn print_ascisstablehdu_struct(hdu: fits::HDU<AsciiTable>) -> Result<(), Box<dyn Error>> {
    print_hdu_type("ASCIITABLE");
    print_asciitable_header(hdu.get_header().get_xtension());
    print_data_struct(&hdu);
    Ok(())
}

fn print_hdu_type(hdu_type: &str) {
    println!(" * HDU type: {}", hdu_type);
}

fn print_data_struct<X>(hdu: &fits::HDU<X>)
where
    X: Xtension + Debug,
{
    let data_starting_byte = hdu.get_data_unit_byte_offset();
    let data_length_byte = hdu.get_data_unit_byte_size();
    println!(
        " * DATA starting byte: {}; byte size: {}.",
        data_starting_byte, data_length_byte
    );
}

fn print_img_header(img: &Image) {
    println!(
        " * HEAD naxis: {}; bitpix : {:?}; dimensions: {}.",
        img.get_naxis(),
        img.get_bitpix(),
        img.get_naxisn_all()
            .iter()
            .map(|d| d.to_string())
            .reduce(|mut s, d| {
                s.push('x');
                s.push_str(&d);
                s
            })
            .unwrap_or_else(|| String::from("0"))
    );
}

fn print_bintable_header(bin: &BinTable) {
    println!(
        " * HEAD n_cols: {}; n_rows: {}.",
        bin.get_num_cols(),
        bin.get_num_rows()
    );
}

fn print_asciitable_header(ascii: &AsciiTable) {
    println!(
        " * HEAD n_cols: {}; n_rows: {}.",
        ascii.get_num_cols(),
        ascii.get_num_rows()
    );
}
