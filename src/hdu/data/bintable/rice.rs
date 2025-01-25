#[derive(Debug)]
pub(crate) struct RICEDecoder<R> {
    pub reader: R
}

use std::io::Read;
impl<R> Read for RICEDecoder<R>
where
    R: Read
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}

impl<R> RICEDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader
        }
    }
}