
use std::str::FromStr;
use std::error::Error;


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
///
/// As a small optimization, this type contains its string data not as
/// `String`, but as `Box<str>`. This shaves off the capacity field of
/// regular `String`s and thus reduces the types stack size by one
/// `usize`.
#[derive(Debug, PartialEq)]
pub enum InputLine {
    /// A comment line. Disregarded completely.
    Comment,
    /// A header line. Contains the part within the brackets.
    Header(Box<str>),
    /// A definition, split at the first equals sign.
    Definition(Definition),
}

impl FromStr for InputLine {
    type Err = SyntaxError;

    /// Parses a line and decide how to interpret it.
    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let line = line.trim();
        if is_comment(line) {
            Ok(InputLine::Comment)
        } else if let Some(name) = try_parse_header(line) {
            Ok(InputLine::Header(Box::from(name?)))
        } else if let Some(equals_sign_pos) = try_parse_definition(line) {
            let line = line.into();
            Ok(
                InputLine::Definition(
                    Definition {
                        line,
                        equals_sign_pos,
                    },
                ),
            )
        } else {
            Err(SyntaxError::NotAVarDef(line.to_owned()))
        }
    }
}

impl InputLine {
    /// Returns `true` if this is a comment line.
    pub fn is_comment(&self) -> bool {
        match *self {
            InputLine::Comment => true,
            _ => false,
        }
    }

    /// Returns `true` if this is a header line.
    pub fn is_header(&self) -> bool {
        match *self {
            InputLine::Header(_) => true,
            _ => false,
        }
    }

    /// Returns `true` if this is a definition line.
    pub fn is_definition(&self) -> bool {
        match *self {
            InputLine::Definition(_) => true,
            _ => false,
        }
    }

    /// If this is a header line, return its contents.
    pub fn header(&self) -> Option<&str> {
        match *self {
            InputLine::Header(ref s) => Some(s),
            _ => None,
        }
    }

    /// If this is a definition line, return its split contents.
    pub fn definition(&self) -> Option<(&str, &str)> {
        match *self {
            InputLine::Definition(ref d) => Some(d.parts()),
            _ => None,
        }
    }

    /// If this is a definition line, return the name it defines.
    pub fn definition_name(&self) -> Option<&str> {
        match *self {
            InputLine::Definition(ref d) => Some(d.name()),
            _ => None,
        }
    }

    /// If this is a definition line, return the assigned value.
    pub fn definition_value(&self) -> Option<&str> {
        match *self {
            InputLine::Definition(ref d) => Some(d.value()),
            _ => None,
        }
    }
}


/// Checks if a line is blank or a comment.
fn is_comment(s: &str) -> bool {
    s.is_empty() || s.starts_with('#')
}


/// Returns the inside of the brackets if `s` is a header line.
///
/// # Errors
/// If `s` is not a header line, this returns `None`.
/// If `s` begins with an opening bracket, but doesn't end with a
/// closing bracket, this returns `Some(Err(err))`.
fn try_parse_header(s: &str) -> Option<Result<&str, SyntaxError>> {
    if !s.starts_with('[') {
        return None;
    }
    if !s.ends_with(']') {
        let err = if s.find(']').is_none() {
            SyntaxError::NoClosingBracket(s.to_owned())
        } else {
            SyntaxError::TextAfterClosingBracket(s.to_owned())
        };
        return Some(Err(err));
    }
    // Should be safe because '[' and ']' are one byte long
    // in UTF-8.
    let inner = s[1..s.len() - 1].trim();
    Some(Ok(inner))
}


/// Returns the position of the equals sign if `s` is a definition.
fn try_parse_definition(s: &str) -> Option<usize> {
    s.find('=')
}


/// Helper type that describes how to split a definition line.
///
/// We _could_ save a definition line simply as a pair of `&str`s (one
/// for the variable name, one for its value), but keeping it like this
/// saves us one `usize` of space. In return, we have to call
/// `str::trim()` every time we want to actually get the variable name
/// and definition.
#[derive(Debug, PartialEq)]
pub struct Definition {
    line: Box<str>,
    equals_sign_pos: usize,
}

impl Definition {
    /// Gets the name of the variable defined in this line.
    pub fn name(&self) -> &str {
        self.line[..self.equals_sign_pos].trim_right()
    }

    /// Gets the value of the variable defined in this line.
    pub fn value(&self) -> &str {
        self.line[self.equals_sign_pos + 1..].trim_left()
    }

    /// Gets both name and value of the variable defined in this line.
    pub fn parts(&self) -> (&str, &str) {
        (self.name(), self.value())
    }
}

quick_error! {
    /// Error caused by a line not adhering to the syntax described in
    /// the documentation for `InputLine`.
    #[derive(Debug)]
    pub enum SyntaxError {
        NoClosingBracket(line: String) {
            description("syntax error: bracket \"[\" not closed in header line")
            display(err) -> ("{}: \"{}\"", err.description(), line)
        }
        TextAfterClosingBracket(line: String) {
            description("syntax error: text after closing bracket \"]\" of a header line")
            display(err) -> ("{}: \"{}\"", err.description(), line)
        }
        NotAVarDef(line: String) {
            description("syntax error: missing equals sign \"=\" in variable definition")
            display(err) -> ("{}: \"{}\"", err.description(), line)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_of_inputline() {
        assert_eq!(::std::mem::size_of::<InputLine>(), 32);
    }

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
            let line = line.parse().unwrap();
            let parts = match line {
                InputLine::Definition(ref def) => def.parts(),
                _ => panic!("not parsed as a definition"),
            };
            assert_eq!(parts, (expected_var, expected_def));
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
