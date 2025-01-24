#[derive(Debug)]
pub(crate) struct RICEDecoder<R> {
    reader: R
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
    pub(crate) fn new(reader: R) -> Self {
        Self {
            reader
        }
    }

    pub(crate) fn into_inner(self) -> R {
        self.reader
    }
}