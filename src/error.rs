quick_error! {
    #[derive(Debug, PartialEq)]
    pub enum Error {
        /// General error case
        StaticError(message: &'static str) {
            from()
            display("{}", message)
        }
        DynamicError(message: String) {
            from()
            display("{}", message)
        }
        BitpixBadValue {
            display("Bitpix value found is not valid. Standard values are: -64, -32, 8, 16, 32 and 64.")
        }
        /// Fits file is not a multiple of 80 bytes long
        FailReadingNextBytes {
            display("A 80 bytes card could not be read. A fits file must have a multiple of 80 characters.")
        }
        FailFindingKeyword(keyword: String) {
            display("{} keyword has not been found.", keyword)
        }
        ValueBadParsing {
            display("A value could not be parsed correctly")
        }
        FailTypeCardParsing(card: String, t: String) {
            display("{} card is not of type {}", card, t)
        }
        NotSupportedXtensionType(extension: String) {
            display("`{}` extension is not supported. Only BINTABLE, TABLE and IMAGE are.", extension)
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
