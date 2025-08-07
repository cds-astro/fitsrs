quick_error! {
    #[derive(Debug, PartialEq)]
    pub enum Error {
        /// General error case
        StaticError(message: &'static str) {
            from()
            display("{message}")
        }
        DynamicError(message: String) {
            from()
            display("{message}")
        }
        /// Fits file is not a multiple of 80 bytes long
        FailReadingNextBytes {
            display("A 80 bytes card could not be read. A fits file must have a multiple of 80 characters.")
        }
        FailFindingKeyword(keyword: String) {
            display("{keyword} keyword has not been found.")
        }
        WCS {
            from(wcs::error::Error)
            display("WCS parsing")
        }
        NotSupportedXtensionType(extension: String) {
            display("{extension} extension is not supported. Only BINTABLE, TABLE and IMAGE are.")
        }
        Utf8 {
            from(std::str::Utf8Error)
            display("Fail to parse a keyword as a utf8 string")
        }
        /// IO error wrapping the std::io::Error
        Io(kind: std::io::ErrorKind) {
            // to be able to derive from PartialEq just above
            // as std::io::Error does not impl PartialEq I decided
            // to only store its error kind which is sufficiant for our use
            from(err: std::io::Error) -> (err.kind())
        }
    }
}

impl serde::de::Error for Error {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Error::DynamicError(msg.to_string())
    }
}
