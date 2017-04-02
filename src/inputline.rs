
use std::io;


/// Type that defines how each line of an input file is interpreted.
///
/// Input files are read line by line and each line can be either:
///
/// 1. A header line, surrounded by brackets; or
/// 2. A definition line, containing an equals sign;
///
/// Anything else is considered a syntax error.
///
/// Additionally, lines whose first non-whitespace characters is a pound
/// sign are intepreted as comments and ignored completely. Furthermore,
/// all leading and trailing whitespace is stripped before interpretation
/// begins.
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
    /// No input file was retrieved.
    None,
    /// A header. Contains the part within the brackets.
    Header(String),
    /// A definition. Contains the part before and after the equal sign.
    Definition(String, String),
    /// An error. Contains the offending line.
    SyntaxError(String),
}

impl InputLine {
    /// Gets the next non-comment input line from an iterator of lines.
    ///
    /// This iterates over the input, dropping all blank or comment
    /// lines. The first non-comment line then is parsed as an
    /// `InputLine` and returned. The iterator may be used further
    /// after the call.
    ///
    /// If the iterator is exhausted without finding another
    /// non-comment line, `InputLine::None` is returned.
    pub fn from_iter<I>(lines: &mut I) -> Self where I: Iterator<Item=String> {
        for line in lines {
            if let Some(line) = Self::from_line(&line) {
                return line;
            }
        }
        InputLine::None
    }

    /// Like `from_iter()`, but meant for use with `BufRead::lines()`.
    pub fn from_io<I>(lines: &mut I) -> io::Result<Self>
        where I: Iterator<Item=io::Result<String>>
    {
        for line in lines {
            if let Some(line) = Self::from_line(&line?) {
                return Ok(line);
            }
        }
        Ok(InputLine::None)
    }

    /// Parses a line and decide how to interpret it.
    ///
    /// If the passed line is blank or a comment (ignoring surrounding
    /// whitespace), this function returns `None`. Otherwise, it
    /// returns some interpreted result.
    fn from_line(line: &str) -> Option<Self> {
        let line = line.trim();
        if Self::is_ignorable(line) {
            None
        } else if let Some(name) = Self::parse_header(line) {
            Some(InputLine::Header(name.to_owned()))
        } else if let Some((name, value)) = Self::parse_definition(line) {
            Some(InputLine::Definition(name.to_owned(), value.to_owned()))
        } else {
            Some(InputLine::SyntaxError(line.to_owned()))
        }
    }

    /// Checks if a line is blank or a comment.
    fn is_ignorable(s: &str) -> bool { s.is_empty() || s.starts_with('#') }

    /// If `s` is a header line, return the contents of the brackets.
    ///
    /// If `s` is not a header line, return `None`.
    fn parse_header(s: &str) -> Option<&str> {
        if s.starts_with('[') && s.ends_with(']') && s.len() > 2 {
            Some(s[1..s.len()-1].trim())
        } else {
            None
        }
    }

    /// If `s` is a definition, return the split contents.
    ///
    /// If `s` is not a header line, return `None`.
    fn parse_definition(s: &str) -> Option<(&str, &str)> {
        if let Some(n) = s.find('=') {
            let (name, value) = s.split_at(n);
            Some((name.trim(), value[1..].trim()))
        } else {
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec() {
        let input = vec![
            "[test]",
            "first = 1",
            "second = 2",
            "third = 3",
            "",
            "",
            "# comment",
            "[test]",
        ];
        let mut it = input.into_iter().map(String::from);
        assert_eq!(InputLine::from_iter(&mut it), InputLine::Header("test".into()));
        assert_eq!(InputLine::from_iter(&mut it),
                   InputLine::Definition("first".into(), "1".into()));
        assert_eq!(InputLine::from_iter(&mut it),
                   InputLine::Definition("second".into(), "2".into()));
        assert_eq!(InputLine::from_iter(&mut it),
                   InputLine::Definition("third".into(), "3".into()));
        assert_eq!(InputLine::from_iter(&mut it), InputLine::Header("test".into()));
    }
}
