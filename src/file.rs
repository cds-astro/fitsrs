use std::fs::File;
use std::io::BufReader;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use crate::gz::GzReader;
use crate::Fits;

#[derive(Debug)]
pub struct FITSFile(Fits<GzReader<BufReader<File>>>);

use std::fmt::Debug;

use crate::error::Error;
impl FITSFile {
    /// Open a fits file from a path. Can be gzip-compressed
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let f = File::open(path)?;
        let bufreader = BufReader::new(f);
        // Decorate the reader with a gz decoder
        let reader = GzReader::new(bufreader)?;
        Ok(Self(Fits::from_reader(reader)))
    }
}

impl Deref for FITSFile {
    type Target = Fits<GzReader<BufReader<File>>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for FITSFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
