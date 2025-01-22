use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use crate::gz::GzReader;
use crate::Fits;

/// This offers a method to open a file and provide a HDU list iterator over it
/// 
/// The opening process handle externally gzipped files
/// 
/// The downside is that the GzReader does not impl Seek, even if the original file is not gzipped
/// Therefore, seek method will not be used to get to the next HDU after parsing an HDU
/// If you know that your file is not externally gzipped, then you can directly use the Fits::from_reader method and providing
/// it a Seekable reader
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
