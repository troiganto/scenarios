
use std::str::FromStr;
use std::error::Error;
use std::fmt::{self, Display};


/// Type that defines how each line of an input file is interpreted.
///
/// Input files are read line by line. Surrounding whitespace is
/// stripped from all lines before processing. Each line can be one
/// of the following:
///
/// 1. if it is blank or it starts with a hash sign `#`, it is a
///    comment;
/// 2. if it is surrounded by square brackets `[` and `]`, it is a
///    header line;
/// 3. if it contains at least one equals sign, it is a definition
///    line.
///
/// Anything else is considered a syntax error.
///
/// # Example
///
/// ```
/// [This is a header line]
/// definition = value
/// other definition = more values
///
/// # Comment line, ignored completely
/// [A new header line]
/// more definitions = cool
/// a syntax error
/// ```
#[derive(Debug, PartialEq)]
pub enum InputLine {
    /// A comment line. Disregarded completely.
    Comment,
    /// A header line. Contains the part within the brackets.
    Header(String),
    /// A definition. Contains the part before and after the first
    /// equal sign.
    Definition(String, String),
}

impl FromStr for InputLine {
    type Err = SyntaxError;

    /// Parses a line and decide how to interpret it.
    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let line = line.trim();
        if Self::is_comment(line) {
            Ok(InputLine::Comment)
        } else if let Some(name) = Self::try_parse_header(line) {
            Ok(InputLine::Header(name.to_owned()))
        } else if let Some((name, value)) = Self::try_parse_definition(line) {
            Ok(InputLine::Definition(name.to_owned(), value.to_owned()))
        } else {
            Err(SyntaxError(line.to_owned()))
        }
    }
}

impl InputLine {
    /// Checks if a line is blank or a comment.
    fn is_comment(s: &str) -> bool {
        s.is_empty() || s.starts_with('#')
    }

    /// If `s` is a header line, return the contents of the brackets.
    ///
    /// If `s` is not a header line, return `None`.
    fn try_parse_header(s: &str) -> Option<&str> {
        if s.starts_with('[') && s.ends_with(']') {
            // Should be safe because '[' and ']' are one byte long
            // in UTF-8.
            Some(s[1..s.len() - 1].trim())
        } else {
            None
        }
    }

    /// If `s` is a definition, return the split contents.
    ///
    /// If `s` is not a header line, return `None`.
    fn try_parse_definition(s: &str) -> Option<(&str, &str)> {
        if let Some(n) = s.find('=') {
            let (name, value_and_equals) = s.split_at(n);
            let value = &value_and_equals[1..];
            Some((name.trim(), value.trim()))
        } else {
            None
        }
    }
}


/// Error caused by a line not adhering to the syntax described in
/// the documentation for `InputLine`.
#[derive(Debug)]
pub struct SyntaxError(String);

impl Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: \"{}\"", self.description(), self.0)
    }
}

impl Error for SyntaxError {
    fn description(&self) -> &str {
        "could not parse line"
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header() {
        fn assert_eq_header(line: &str, expected_header: &str) {
            assert_eq!(
                line.parse::<InputLine>().unwrap(),
                InputLine::Header(expected_header.into())
            );
        }
        assert_eq_header("[Header]", "Header");
        assert_eq_header(" [  Whitespaced\tHeader  ]\n\n", "Whitespaced\tHeader");
        assert_eq_header("[Header = with = equals]", "Header = with = equals");
        assert_eq_header("[#Pound sign header]", "#Pound sign header");
        assert_eq_header("[]", "");
        assert!("[Bad header".parse::<InputLine>().is_err());
    }


    #[test]
    fn test_definition() {
        fn assert_eq_vardef(line: &str, expected_var: &str, expected_def: &str) {
            assert_eq!(
                line.parse::<InputLine>().unwrap(),
                InputLine::Definition(expected_var.into(), expected_def.into())
            );
        }
        assert_eq_vardef("var=def", "var", "def");
        assert_eq_vardef("var = def", "var", "def");
        assert_eq_vardef("   var\n=\ndef\t", "var", "def");
        assert_eq_vardef("var = def = def", "var", "def = def");
        assert_eq_vardef("var = #def", "var", "#def");
        assert_eq_vardef("v#ar = def", "v#ar", "def");
        assert_eq_vardef("var = [def]", "var", "[def]");
        assert_eq_vardef("var[ = ]def", "var[", "]def");
        assert_eq_vardef("var=", "var", "");
        assert_eq_vardef("=#def", "", "#def");
        assert_eq_vardef("=", "", "");
        assert!("var!".parse::<InputLine>().is_err());
    }


    #[test]
    fn test_comment() {
        fn assert_eq_comment(line: &str) {
            assert_eq!(line.parse::<InputLine>().unwrap(), InputLine::Comment);
        }
        assert_eq_comment("# comment");
        assert_eq_comment("#comment");
        assert_eq_comment("\n\t#comment");
        assert_eq_comment("#[header]");
        assert_eq_comment("#[header");
        assert_eq_comment("#var=def");
        assert_eq_comment("#");
        assert_eq_comment("");
        assert_eq_comment("\t\t\t");
    }
}
