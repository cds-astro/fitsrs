use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use crate::gz::GzReader;
use crate::Fits;

#[derive(Debug)]
pub struct FITSFile;

use std::fmt::Debug;

use crate::error::Error;
impl FITSFile {
    /// Open a fits file from a path. Can be gzip-compressed
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Fits<GzReader<BufReader<File>>>, Error> {
        let f = File::open(path)?;
        let bufreader = BufReader::new(f);
        // Decorate the reader with a gz decoder
        let reader = GzReader::new(bufreader)?;
        Ok(Fits::from_reader(reader))
    }
}
