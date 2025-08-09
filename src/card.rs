use std::{char::REPLACEMENT_CHARACTER, convert::TryFrom};

use crate::{error::Error, hdu::header::extension::XtensionType};
/// Holds eight bytes of ASCII characters, i.e. the length of a FITS compliant keyword.
pub type Keyword = [u8; 8];
/// Holds 80 bytes of ASCII characters, i.e. one line in a FITS compliant file.
pub type CardBuf = [u8; 80];

use serde::Serialize;

/// Enum representing the variants of a single 80 character line in the header.
#[derive(PartialEq, Debug, Serialize, Clone)]
pub enum Card {
    /// A keyword card with a value and optional comment.
    Value { name: String, value: Value },
    /// Continuation of a long-string with `CONTINUE`` in the keyword field of the card.
    Continuation {
        string: Option<String>,
        comment: Option<String>,
    },
    // FITS extension string value, cf FITSv4, section 4.2.1.1
    Xtension {
        x: XtensionType,
        comment: Option<String>,
    },
    /// A file comment where the keyword field is `COMMENT` or an empty string.
    Comment(String),
    /// A log line of the operations used for processing the file.
    History(String),
    /// A keyword card, with a value and optional comment, using the HIERARCH convention.
    Hierarch { name: String, value: Value },
    /// An empty line at the end of the header block providing header space for adding new cards or used for aesthetic purposes.
    Space,
    /// End marker.
    End,
    /// A card with an undefined FITS structure, possibly in violation with the FITS standard.
    Undefined(String),
}

impl Card {
    /// Returns `true` if and only if the the card is either a [value](Card::Value) card with a
    /// [string](Value::String) value or a [hierarch](Card::Hierarch) card with a string](Value::String),
    /// or a [continuation](Card::Continuation) where the string value ends with an ampersand,
    /// i.e. the `&` character.
    pub fn continued(&self) -> bool {
        match self {
            Card::Value {
                value: Value::String { value: s, .. },
                ..
            }
            | Card::Hierarch {
                value: Value::String { value: s, .. },
                ..
            }
            | Card::Continuation {
                string: Some(s), ..
            } => s.ends_with('&'),
            _ => false,
        }
    }

    /// Append a [Card::Continuation] to this [Card] if it is a [Card::Value] of
    /// type [Value::String] and if the string is
    /// [continued][Self::continued()], else panics.
    /// ```
    ///     # use fitsrs::card:: { Card, Value };
    ///     # use std::convert::TryFrom;
    ///     let cards = [
    ///         b"STRKEY  = 'This keyword value is continued&'                                    ",
    ///         b"CONTINUE ' over multiple keyword cards.  '                                      ",
    ///     ];
    ///
    ///     let mut card = Card::try_from(cards[0]).unwrap();
    ///
    ///     card.append(&Card::try_from(cards[1]).unwrap());
    /// ```
    pub fn append(&mut self, r: &Self) -> &mut Self {
        assert!(self.continued(), "card must hold a continued string value");

        if let Self::Continuation {
            string: cont_value,
            comment: cont_comment,
        } = r
        {
            match self {
                Self::Value { value, .. } | Self::Hierarch { value, .. } => {
                    value.append(cont_value, cont_comment);
                }
                _ => panic!("card must be a value or a hierarch"),
            }
        } else {
            panic!("only continuation variants can be appended")
        }
        self
    }

    /// Version of [Card::append] that consumes `self`, useful for chaining operations when
    ///  reconstructing a continued string.
    /// ```
    /// # use std::error::Error;
    /// # fn main() -> Result<(), Box<dyn Error>> {
    /// #   use fitsrs::card:: { Card, Value };
    /// #   use std::convert::TryFrom;
    ///
    ///     let cards = [
    ///         b"STRKEY  = 'This keyword value is continued&'                                    ",
    ///         b"CONTINUE ' over multiple keyword cards.  '                                      ",
    ///     ];
    ///
    ///     // Chained operation when reconstructering a long string.
    ///     let card = Card::try_from(cards[0])?.splice(Card::try_from(cards[1])?);
    /// #   Ok(())
    /// # }
    pub fn splice(mut self, r: Self) -> Self {
        self.append(&r);
        self
    }
}

fn append_string(value: &mut String, v: &Option<String>) {
    debug_assert!(
        value.ends_with("&"),
        "appending value to an uncontinued string"
    );
    value.truncate(value.len() - 1); // remove the trailing ampersand

    if let Some(v) = v {
        if v.is_empty() || v == " " {
            *value = value.trim_end().to_owned(); // remove trailing space before & char
        } else {
            value.push_str(v);
        }
    } else {
        *value = value.trim_end().to_owned();
    }
}

fn append_comment(comment: &mut String, c: &Option<String>) {
    if let Some(c) = c {
        comment.push('\n');
        comment.push_str(c);
    }
}

impl TryFrom<&CardBuf> for Card {
    type Error = Error;

    fn try_from(buf: &CardBuf) -> Result<Self, Self::Error> {
        let kw = std::str::from_utf8(buf[..8].trim_ascii())?;
        match kw {
            "" => Ok(parse_empty_keyword_card(buf)),
            "COMMENT" => Ok(Card::Comment(parse_comment_text(&buf[8..]))),
            "HISTORY" => Ok(Card::History(parse_comment_text(&buf[8..]))),
            "CONTINUE" => parse_continuation(buf),
            "XTENSION" => parse_extension(buf),
            "HIERARCH" => parse_hierarch(buf),
            "END" => Ok(Card::End),
            _ => {
                if b"= " == &buf[8..10] {
                    Ok(Card::Value {
                        name: kw.to_owned(),
                        value: parse_value(&buf[10..])?,
                    })
                } else {
                    // FIXME add a warn here to tell the user this may be a case from a convention that we
                    // do not support (e.g. the HIERARCH convention)
                    Ok(Card::Undefined(String::from_utf8_lossy(buf).into_owned()))
                }
            }
        }
    }
}

fn parse_extension(buf: &[u8; 80]) -> Result<Card, Error> {
    let (value, comment) = split_value_and_comment(&buf[10..])?;
    if value.starts_with("'") && value.ends_with("'") {
        let end = value.len() - 1;
        let x = XtensionType::try_from(value[1..end].trim_ascii())?;
        Ok(Card::Xtension { x, comment })
    } else {
        let msg = format!("XTENSION value must be enclosed in single quotes, found: {value}");
        Err(Error::DynamicError(msg))
    }
}

fn parse_hierarch(buf: &[u8; 80]) -> Result<Card, Error> {
    // Starts at 10 to remove the leading `HIERARCH  `
    if let Some(mut index_eq) = &buf[9..].iter().position(|&b| b == b'=') {
        index_eq += 9;
        if let Some(kw) = &buf[9..index_eq]
            .split(|b| b.is_ascii_whitespace())
            .filter(|s| !s.is_empty())
            .map(String::from_utf8_lossy)
            .reduce(|mut acc, e| {
                acc += ".";
                acc += e;
                acc
            })
        {
            Ok(Card::Hierarch {
                name: kw.to_string(),
                value: parse_value(&buf[index_eq + 1..])?,
            })
        } else {
            let kwr = String::from_utf8_lossy(buf);
            let msg = format!("Empty keyword in HIERARCH keyword record \"{kwr}\"");
            Err(Error::DynamicError(msg))
        }
    } else {
        let kwr = String::from_utf8_lossy(buf);
        let msg = format!("Key/value separator '=' not found in HIERARCH keyword record \"{kwr}\"");
        Err(Error::DynamicError(msg))
    }
}

/// FITSv4, sections 4.2. Value and 4.1.2.3. Value/comment (Bytes 11 through 80)
fn parse_value(buf: &[u8]) -> Result<Value, Error> {
    let (v, c) = split_value_and_comment(buf)?;
    if let Some(ch) = v.chars().next() {
        match ch {
            '\'' => Ok(Value::String {
                value: parse_string(v)?,
                comment: c,
            }),
            'T' => Ok(Value::Logical {
                value: true,
                comment: c,
            }),
            'F' => Ok(Value::Logical {
                value: false,
                comment: c,
            }),
            '(' => Err(Error::StaticError("Complex values not yet supported")),
            '0'..='9' | '-' | '+' | '.' => parse_number(v, c),
            _ => Ok(Value::Invalid(String::from_utf8_lossy(buf).into_owned())),
        }
    } else {
        Ok(Value::Undefined)
    }
}

/// Try parse a [Value::Float] falling back parsing a [Value::Integer] and return [Value::Undefined]
/// if the value consists only of empty space. Else return [Err].
fn parse_number(v: String, c: Option<String>) -> Result<Value, Error> {
    if v.is_empty() {
        Ok(Value::Undefined) // FITSv4, section 4.1.2.3
    } else if let Ok(val) = v.parse::<i64>() {
        // First parse integer
        Ok(Value::Integer {
            value: val,
            comment: c,
        })
    } else if let Ok(val) = v.parse::<f64>() {
        // If it fails try parsing a float
        Ok(Value::Float {
            value: val,
            comment: c,
        })
    } else {
        // fallback to D as an exponent, cf. FITSv4, section 4.2.4 Real floating-point number
        let v = v.replace('D', "E");
        if let Ok(val) = v.parse::<f64>() {
            Ok(Value::Float {
                value: val,
                comment: c,
            })
        } else {
            Ok(Value::Invalid(v))
        }
    }
}

/// FITSv4 section 4.2.1.1: `null` string contains no space, `empty` string is one or more space
/// trimmed to one space.
fn parse_string(s: String) -> Result<String, Error> {
    let start_quote = s.starts_with("'");
    let end_quote = s.ends_with("'");
    let value: &str = match (start_quote, end_quote) {
        (true, true) => &s[1..s.len() - 1], // string enclosed in single quotes
        (false, false) => &s,               // comment string has no quotes
        (true, false) => return Err(Error::StaticError("missing single quote at end")),
        (false, true) => return Err(Error::StaticError("missing single quote at start")),
    };

    if value.is_empty() {
        Ok(value.to_owned()) // null FITS string
    } else {
        let value = value.trim_end().to_owned();
        if value.is_empty() {
            Ok(" ".to_owned()) // empty FITS string
        } else {
            Ok(value)
        }
    }
}

/// Split a value card into its *trimmed* value and an optional comment sepearated
/// by a slash.
///
/// * FITS `string` values are enclosed in single quotes with any escaped quotes replaced
///   by a single single-quote character.
/// * An error is returned if a *non-string* value contains invalid FITS characters.
/// * Invalid FITS characters are replaced with the � characacter (lossy conversion) in
///   strings and comments.
///
/// Example:
/// ```
///     use fitsrs::card::split_value_and_comment;
///     let r = b"''string' with \t, /, and ''leading'' and ''trailing'' ''' / \nand a comment    ";
///     let Ok((v,c)) = split_value_and_comment(r) else { unreachable!("split returned error") };
///     assert_eq!(v, "''string' with �, /, and 'leading' and 'trailing' ''");
///     assert_eq!(c, Some(" �and a comment".to_string()));
/// ```
pub fn split_value_and_comment(buf: &[u8]) -> Result<(String, Option<String>), Error> {
    const UNDEFINED: usize = usize::MAX; // unlikely that we have indexes of usize::MAX...
    let mut slash = UNDEFINED;
    let mut tic = false;
    let mut toc = false;

    let mut value = String::new();
    let buf = String::from_utf8_lossy(buf.trim_ascii());

    // parse value until first slash outside of a string value
    for (i, c) in buf.chars().enumerate() {
        match c {
            '\'' => {
                // quote
                match (tic, toc) {
                    (true, true) => toc = false, // escaped single quote, reset toc
                    (true, false) => {
                        toc = true;
                        value.push('\'')
                    } // end of string or escaped quote
                    (false, false) => {
                        tic = true;
                        value.push('\'')
                    } // beginning of string value
                    (false, true) => unreachable!("there must not be a toc without a tic"), // algorithmic failure
                }
            }
            '/' => {
                // slash
                match (tic, toc) {
                    (true, false) => value.push('/'), // slash inside string, add it
                    (true, true) | (false, false) => {
                        slash = i;
                        break;
                    } // found a slash outside of the string, we're done here
                    (false, true) => unreachable!("there must not be a toc without a tic"), // algorithmic failure
                }
            }
            ' '..='~' => {
                // 0x20..=0x7E, FITSv4, section 4.2.1.1
                // valid FITS characters, add these even if outside string
                if toc {
                    tic = false;
                    toc = false
                } // end of string
                value.push(c)
            }
            _ => {
                // invalid FITS characters, return error if outside the comment else substitute
                if tic {
                    value.push(char::REPLACEMENT_CHARACTER)
                } else {
                    return Err(Error::StaticError("invalid character in value"));
                }
            }
        }
    }

    if slash == UNDEFINED {
        Ok((value.trim().to_owned(), None))
    } else {
        let mut comment = String::new();
        // Replace non-FITS characters in comment with '�'.
        for c in buf.chars().skip(slash + 1) {
            // skip the slash
            match c {
                ' '..='~' => comment.push(c),
                _ => comment.push(char::REPLACEMENT_CHARACTER),
            }
        }
        Ok((value.trim().to_owned(), Some(comment.trim_end().to_owned())))
    }
}

fn parse_continuation(buf: &[u8; 80]) -> Result<Card, Error> {
    let (value, comment) = split_value_and_comment(&buf[8..])?;
    let string = Some(parse_string(value)?);
    Ok(Card::Continuation { string, comment })
}

fn parse_comment_text(buf: &[u8]) -> String {
    let mut comment = String::new();
    buf.iter()
        .map(|b| match b {
            0x20..=0x7E => {
                // FITSv4, section 4.2.1.1
                *b as char
            }
            _ => REPLACEMENT_CHARACTER,
        })
        .for_each(|ch| comment.push(ch));
    comment.trim_ascii_end().to_owned()
}

/// Returns a [Card::Comment] if the card contains text, else [Card::Space].
///
/// FITSv4, section 4.4.2.4. Commentary keywords, last two paragraphs.
fn parse_empty_keyword_card(buf: &[u8; 80]) -> Card {
    let c = parse_comment_text(&buf[8..]);
    if c.is_empty() {
        Card::Space
    } else {
        Card::Comment(c)
    }
}

pub trait CardValue {
    fn parse(value: Value) -> Result<Self, Error>
    where
        Self: Sized;
}

impl CardValue for f64 {
    fn parse(value: Value) -> Result<Self, Error> {
        Value::check_for_float(value)
    }
}
impl CardValue for i64 {
    fn parse(value: Value) -> Result<Self, Error> {
        Value::check_for_integer(value)
    }
}
impl CardValue for String {
    fn parse(value: Value) -> Result<Self, Error> {
        Value::check_for_string(value)
    }
}
impl CardValue for bool {
    fn parse(value: Value) -> Result<Self, Error> {
        Value::check_for_boolean(value)
    }
}

/// Enum structure corresponding to all the possible types of cards in a header.
#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum Value {
    /// FITSv4, section 4.2.3 Integer number
    Integer { value: i64, comment: Option<String> },
    /// FITSv4, section 4.2.4 Real floating-point number
    Float { value: f64, comment: Option<String> },
    /// FITSv4, section 4.2.2 Logical
    Logical {
        value: bool,
        comment: Option<String>,
    },
    /// FITSv4, section 4.2.1 Character string
    /// Single quote enclosed string (4.2.1.1) or continued string (4.2.1.2).
    String {
        value: String,
        comment: Option<String>,
    },
    /// Value field consisting entirely of blank space.
    ///
    /// FITSv4, section 4.1.2.3. Value/comment (Bytes 11 through 80)
    Undefined,
    /// Illegal FITS value or a value consisting entirely blank space, the value is the
    /// UTF8 string with replacement characters.
    ///
    /// FITSv4, section 4.1.2.3. Value/comment (Bytes 11 through 80)
    Invalid(String),
}

impl Value {
    pub fn unit(&self) -> Option<&str> {
        match self {
            Value::Integer { comment, .. }
            | Value::Float { comment, .. }
            | Value::String { comment, .. }
            | Value::Logical { comment, .. } => parse_unit(comment),
            _ => None,
        }
    }

    pub(crate) fn continued(&self) -> bool {
        if let Value::String { value, .. } = self {
            value.ends_with('&')
        } else {
            false
        }
    }
}

/// Return the unit enclosed in brackets of the a comment, returns None
/// if there is no comment or no unit in brackets.
///
/// FITSv4 section 4.3.2. Units in comment fields
fn parse_unit(comment: &Option<String>) -> Option<&str> {
    if let Some(c) = comment {
        if c.starts_with("[") {
            if let Some(i) = c.find("]") {
                Some(&c[1..i])
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

impl Value {
    pub fn check_for_integer(self) -> Result<i64, Error> {
        match self {
            Value::Integer { value: num, .. } => Ok(num),
            _ => Err(Error::ValueBadParsing),
        }
    }
    pub fn check_for_boolean(self) -> Result<bool, Error> {
        match self {
            Value::Logical { value: logical, .. } => Ok(logical),
            _ => Err(Error::ValueBadParsing),
        }
    }
    pub fn check_for_string(self) -> Result<String, Error> {
        match self {
            Value::String { value: s, .. } => Ok(s),
            _ => Err(Error::ValueBadParsing),
        }
    }
    pub fn check_for_float(self) -> Result<f64, Error> {
        match self {
            Value::Float { value: f, .. } => Ok(f),
            _ => Err(Error::ValueBadParsing),
        }
    }

    pub fn append(&mut self, v: &Option<String>, c: &Option<String>) -> &mut Self {
        if let Value::String { value, comment } = self {
            append_string(value, v);
            if let Some(comment) = comment {
                append_comment(comment, c);
            } else {
                *comment = c.clone();
            }
        } else {
            panic!("self is not a string variant")
        }
        self
    }

    pub fn splice(mut self, v: Option<String>, c: Option<String>) -> Self {
        self.append(&v, &c);
        self
    }
}

#[cfg(test)]
mod tests {
    use core::panic;
    use std::convert::TryFrom;

    use crate::{
        card::{parse_empty_keyword_card, parse_string},
        error::Error,
        hdu::header::extension::XtensionType,
    };

    use super::{parse_number, split_value_and_comment, Card, CardBuf, Value};

    #[test]
    fn strings() -> Result<(), Error> {
        assert_eq!(
            "",
            &parse_string("''".to_owned())?,
            "null FITS string should return empty string"
        );
        assert_eq!(
            "",
            &parse_string("".to_owned())?,
            "null FITS comment should return empty string"
        );
        assert_eq!(
            " ",
            &parse_string("' '".to_owned())?,
            "empty FITS string should return single space"
        );
        assert_eq!(
            " ",
            &parse_string(" ".to_owned())?,
            "empty FITS comment should return single space"
        );
        assert_eq!(
            " ",
            &parse_string("'     '".to_owned())?,
            "empty FITS string should return single space"
        );
        assert_eq!(
            " ",
            &parse_string("        ".to_owned())?,
            "empty FITS comment should return single space"
        );

        assert_eq!(
            "some string",
            &parse_string("'some string'".to_owned())?,
            "quotes should be removed"
        );
        assert_eq!(
            "some string",
            &parse_string("'some string   '".to_owned())?,
            "trailing space should be rmeoved"
        );
        assert_eq!(
            "some string",
            &parse_string("some string   ".to_owned())?,
            "trailing space should be rmeoved"
        );
        assert_eq!(
            "'some' 'string'",
            &parse_string("''some' 'string''".to_owned())?,
            "quotes should be preserved"
        );
        assert_eq!(
            "    some string",
            &parse_string("'    some string'".to_owned())?,
            "leading space should be kept"
        );
        assert_eq!(
            "    some string",
            &parse_string("    some string".to_owned())?,
            "leading space should be kept"
        );

        assert!(matches!(
            parse_string("'missing end quote".to_owned()),
            Err(Error::StaticError(_))
        ));
        assert!(matches!(
            parse_string("missing start quote'".to_owned()),
            Err(Error::StaticError(_))
        ));
        Ok(())
    }

    #[test]
    fn number_values() -> Result<(), Error> {
        assert_eq!(
            parse_number("42".to_owned(), None),
            Ok(Value::Integer {
                value: 42,
                comment: None
            })
        );
        assert_eq!(
            parse_number("42.".to_owned(), None),
            Ok(Value::Float {
                value: 42f64,
                comment: None
            })
        );
        assert_eq!(
            parse_number(".42".to_owned(), None),
            Ok(Value::Float {
                value: 0.42f64,
                comment: None
            })
        );
        assert_eq!(
            parse_number("4E2".to_owned(), None),
            Ok(Value::Float {
                value: 4E2f64,
                comment: None
            })
        );
        assert_eq!(
            parse_number("4D2".to_owned(), None),
            Ok(Value::Float {
                value: 4E2f64,
                comment: None
            })
        );
        Ok(())
    }

    /// Assert keyword, value, and comment parsed from the provided card buffer.
    fn assert_string_kvc(buf: &CardBuf, kw: &str, v: &str, c: Option<String>) {
        if let Card::Value {
            name,
            value: Value::String { value, comment },
        } = Card::try_from(buf).unwrap()
        {
            assert_eq!(name, kw);
            assert_eq!(value, v);
            assert_eq!(comment, c);
        } else {
            panic!("expected keyword/value card")
        }
    }

    fn assert_value_comment(r: &str, v: &str, c: Option<&str>) -> Result<(), Error> {
        let (av, ac) = split_value_and_comment(r.as_bytes())?;
        assert_eq!(av, v, "values do not match");
        if let Some(c) = c {
            assert_eq!(ac, Some(c.to_string()), "commments do not match")
        } else {
            assert_eq!(ac, None, "expected 'no comment'")
        }
        Ok(())
    }

    #[test]
    fn comment_card() -> Result<(), Error> {
        let buf =
            b"COMMENT comment starts / ends here...\n                                          ";
        let card = Card::try_from(buf)?;
        if let Card::Comment(comment) = card {
            assert_eq!(comment, "comment starts / ends here...�");
        } else {
            return Err(Error::DynamicError(format!("{card:?}")));
        }
        Ok(())
    }

    #[test]
    fn forward_slash() -> Result<(), Error> {
        assert_value_comment(
            "'String value without comment'",
            "'String value without comment'",
            None,
        )?;
        assert_value_comment(
            "'' / Comment with null string",
            "''",
            Some(" Comment with null string"),
        )?;
        assert_value_comment(
            "' ' / Comment with empty string",
            "' '",
            Some(" Comment with empty string"),
        )?;
        assert_value_comment(
            "'String value ' / and a comment",
            "'String value '",
            Some(" and a comment"),
        )?;
        assert_value_comment(
            "'String value with embedded /' / and a comment",
            "'String value with embedded /'",
            Some(" and a comment"),
        )?;

        assert_value_comment(
            "    'Free-string'     /   with comment",
            "'Free-string'",
            Some("   with comment"),
        )?;

        Ok(())
    }

    #[test]
    fn invalid_characters() -> Result<(), Error> {
        assert_value_comment("'String with \n newline'", "'String with � newline'", None)?;
        assert_value_comment(
            "'😈' / The devil is in the details!",
            "'�'",
            Some(" The devil is in the details!"),
        )?;
        assert_value_comment(
            "''Just' smile 😀' / with \n newline and \t tab in comment",
            "''Just' smile �'",
            Some(" with � newline and � tab in comment"),
        )?;

        Ok(())
    }

    #[test]
    fn string_tic_escape() -> Result<(), Error> {
        assert_value_comment(
            "'Escaped ''tics''' / comment",
            "'Escaped 'tics''",
            Some(" comment"),
        )
    }

    /// Ampersand should be preserved after missing CONTINUE.
    ///
    /// FITSv4 section 4.2.1.2 - Continued string (long-string) keywords; last paragraph.
    #[test]
    fn trailing_ampersand() {
        let r = b"STRKEY  = 'Trailing ampersand should be preserved&'                             ";
        assert_string_kvc(r, "STRKEY", "Trailing ampersand should be preserved&", None);
    }

    #[test]
    fn xtension_card() {
        let r = b"XTENSION= 'TABLE   ' / an extension table                                       ";
        assert_eq!(
            Card::try_from(r),
            Ok(Card::Xtension {
                x: XtensionType::AsciiTable,
                comment: Some(" an extension table".to_owned())
            })
        );
    }

    #[test]
    fn empty_keyword_card() {
        let r =
            b"        empty header comment with an illegal \t tab and \n newline                ";
        assert_eq!(
            parse_empty_keyword_card(r),
            Card::Comment("empty header comment with an illegal � tab and � newline".to_owned()),
        );

        let r = b"                                                                                ";
        assert_eq!(parse_empty_keyword_card(r), Card::Space);
    }

    #[test]
    fn undefined_card() -> Result<(), Error> {
        let r1 =
            b"SOMEKEY   which is not a value as it does not have '= ' at pos 9 and 10         ";
        let r2 =
            b"SOMEKEY   which is not a value as it does not have '= ' at pos 9 and 10         ";
        assert_eq!(
            Card::try_from(r1)?,
            Card::Undefined(String::from_utf8_lossy(r2).into_owned())
        );
        Ok(())
    }

    #[test]
    fn string_value_trimming() {
        let r = b"STRKEY  = 'Trailing space should be removed.         '                          ";
        assert_string_kvc(r, "STRKEY", "Trailing space should be removed.", None);

        let r = b"STRKEY  = '       Leading space should be preserved.'                           ";
        assert_string_kvc(
            r,
            "STRKEY",
            "       Leading space should be preserved.",
            None,
        );

        let r = b"STRKEY  = ''                                                                    ";
        assert_string_kvc(r, "STRKEY", "", None); // FITS null string has zero length

        let r = b"STRKEY  = '        '                                                            ";
        assert_string_kvc(r, "STRKEY", " ", None); // FITS empty string is collapsed to one space
                                                   // todo: test long trimming
    }

    #[test]
    fn logic_value() {
        let r = b"STRKEY  =                    T / a true statement!                              ";
        let kw = Card::try_from(r).unwrap();

        if let Card::Value {
            name,
            value:
                Value::Logical {
                    value,
                    comment: Some(comment),
                },
        } = kw
        {
            assert_eq!(name, "STRKEY");
            assert!(value);
            assert_eq!(comment, " a true statement!");
        } else {
            panic!("card is not a string keyword or it is missing its comment")
        }
    }

    /// Happy path for continued long-string values and comments.
    ///
    /// 4.2.1.2 Continued string (long-string) keywords
    #[test]
    fn long_string_value() {
        let cards = [
            b"STRKEY  = 'This keyword value is continued&'                                    ",
            b"CONTINUE ' over multiple keyword cards. &'                                      ",
            b"CONTINUE '&' / The comment field for this                                       ",
            b"CONTINUE '&' / keyword is also continued                                        ",
            b"CONTINUE '' / over multiple cards.                                              ",
        ];

        let kw = Card::try_from(cards[0])
            .unwrap()
            .splice(Card::try_from(cards[1]).unwrap())
            .splice(Card::try_from(cards[2]).unwrap())
            .splice(Card::try_from(cards[3]).unwrap())
            .splice(Card::try_from(cards[4]).unwrap());

        if let Card::Value { name, value } = kw {
            assert_eq!(name, "STRKEY");
            if let Value::String { value, .. } = &value {
                assert_eq!(
                    value,
                    "This keyword value is continued over multiple keyword cards."
                );
            } else {
                panic!("Not a Value::String")
            }
            if let Value::String {
                comment: Some(comment),
                ..
            } = &value
            {
                assert_eq!(
                    comment,
                    " The comment field for this\n keyword is also continued\n over multiple cards."
                );
            } else {
                panic!("Comment is None")
            }
        } else {
            panic!("Card is not a Card::Value")
        }
    }

    #[test]
    fn hierarch_keyword_record() {
        let r =
            b"HIERARCH ESO TEL FOCU SCALE = 1.489 / (deg/m) Focus length = 5.36\"/mm           ";
        let kw = Card::try_from(r).unwrap();

        if let Card::Hierarch {
            name,
            value:
                Value::Float {
                    value,
                    comment: Some(comment),
                },
        } = kw
        {
            assert_eq!(name, "ESO.TEL.FOCU.SCALE");
            assert_eq!(value, 1.489_f64);
            assert_eq!(comment, " (deg/m) Focus length = 5.36\"/mm");
        } else {
            panic!("card is not a string keyword or it is missing its comment")
        }
    }
}
