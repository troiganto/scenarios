// Copyright 2017 Nico Madysa.
//
// Licensed under the Apache License, Version 2.0 (the "License"); you
// may not use this file except in compliance with the License. You may
// obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or
// implied. See the License for the specific language governing
// permissions and limitations under the License.


use std::str::FromStr;


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
/// Anything else is considered a syntax error. Use the [`kind()`]
/// method to query which of these kinds an input line is classified
/// as.
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
///
/// [`kind()`]: #method.kind
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InputLine {
    /// The string content of the line.
    ///
    /// For header lines, this is the name of the header, with
    /// surrounding whitespace and the brackets removed. For definition
    /// lines, this is the full line, with surrounding whitespace
    /// removed. For comments, this is `None`.
    content: Option<Box<str>>,
    /// The position of the equal sign inside the line.
    ///
    /// This value is zero for comments and header lines. Only for
    /// definition lines, it is non-zero. It is the index of the equals
    /// sign inside `content` that separates variable name and value.
    ///
    /// Note that header lines may very well contain equals signs.
    /// This field will be zero for them regardless.
    eq_pos: usize,
}

impl FromStr for InputLine {
    type Err = SyntaxError;

    /// Parses a line and decide how to interpret it.
    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let line = line.trim();
        if is_comment(line) {
            let line = InputLine { content: None, eq_pos: 0 };
            Ok(line)
        } else if let Some(name) = try_parse_header(line) {
            let line = InputLine {
                content: Some(Box::from(name?)),
                eq_pos: 0,
            };
            Ok(line)
        } else if let Some(equals_sign_pos) = try_parse_definition(line) {
            let line = InputLine {
                content: Some(Box::from(line)),
                eq_pos: equals_sign_pos?,
            };
            Ok(line)
        } else {
            Err(SyntaxError::NotAVarDef(line.to_owned()))
        }
    }
}

impl InputLine {
    /// Returns `true` if this is a comment line.
    pub fn is_comment(&self) -> bool {
        self.content.is_none()
    }

    /// Returns `true` if this is a header line.
    pub fn is_header(&self) -> bool {
        self.content.is_some() && self.eq_pos == 0
    }

    /// Returns `true` if this is a definition line.
    pub fn is_definition(&self) -> bool {
        self.content.is_some() && self.eq_pos > 0
    }

    /// Returns what kind of input line that this string got parsed as.
    pub fn kind(&self) -> InputLineKind {
        if self.eq_pos > 0 {
            InputLineKind::Definition
        } else if self.content.is_some() {
            InputLineKind::Header
        } else {
            InputLineKind::Comment
        }
    }

    /// If this is a header line, return its contents.
    pub fn header(&self) -> Option<&str> {
        if self.eq_pos == 0 {
            self.content.as_ref().map(Box::as_ref)
        } else {
            None
        }
    }

    /// If this is a definition line, return its split contents.
    pub fn definition(&self) -> Option<(&str, &str)> {
        if self.eq_pos > 0 {
            self.content
                .as_ref()
                .map(|s| (s[..self.eq_pos].trim_right(), s[self.eq_pos + 1..].trim_left()),)
        } else {
            None
        }
    }

    /// If this is a definition line, return the name it defines.
    pub fn definition_name(&self) -> Option<&str> {
        if self.eq_pos > 0 {
            self.content
                .as_ref()
                .map(|line| line[..self.eq_pos].trim_right())
        } else {
            None
        }
    }

    /// If this is a definition line, return the assigned value.
    pub fn definition_value(&self) -> Option<&str> {
        if self.eq_pos > 0 {
            self.content
                .as_ref()
                .map(|line| line[self.eq_pos + 1..].trim_left())
        } else {
            None
        }
    }
}


/// The kinds of [`InputLine`]s that exist.
///
/// [`InputLine`]: ./struct.InputLine.html
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputLineKind {
    /// A header line.
    Header,
    /// A variable definition.
    Definition,
    /// A comment or empty line.
    Comment,
}


/// Checks if a line is empty or a comment.
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
            SyntaxError::MissingClosingBracket(s.to_owned())
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
///
/// # Errors
/// This function returns `Some(Err(SyntaxError))` if the line contains
/// an equals sign, but it is at index `0`, i.e. there is no variable
/// name in front of it.
fn try_parse_definition(s: &str) -> Option<Result<usize, SyntaxError>> {
    match s.find('=') {
        Some(pos) if pos > 0 => Some(Ok(pos)),
        Some(_) => Some(Err(SyntaxError::MissingVariableName(s.to_owned()))),
        None => None,
    }
}


/// Error caused by a line not adhering to the syntax described in
/// the documentation for [`InputLine`].
///
/// [`InputLine`]: ./struct.InputLine.html
#[derive(Debug, Fail)]
pub enum SyntaxError {
    #[fail(display = "no closing bracket \"]\" in header line: \"{}\"", _0)]
    MissingClosingBracket(String),
    #[fail(display = "closing bracket \"]\" does not end the line: \"{}\"", _0)]
    TextAfterClosingBracket(String),
    #[fail(display = "no variable name before \"=\" of a variable definition: \"{}\"", _0)]
    MissingVariableName(String),
    #[fail(display = "no equals sign \"=\" in variable definition: \"{}\"", _0)]
    NotAVarDef(String),
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Expects an error and converts it to string via `Display`.
    fn err_string(line: &str) -> String {
        line.parse::<InputLine>().unwrap_err().to_string()
    }

    #[test]
    fn test_size_of_inputline() {
        use std::mem::size_of;
        assert_eq!(size_of::<InputLine>(), 3 * size_of::<usize>());
    }

    #[test]
    fn test_header() {
        fn assert_eq_header(line: &str, expected_header: &str) {
            let input_line = line.parse::<InputLine>().unwrap();
            if let Some(header) = input_line.header() {
                assert_eq!(header, expected_header);
            } else {
                panic!("not a header: {}", line.to_owned());
            }
            assert_eq!(input_line.kind(), InputLineKind::Header);
        }
        assert_eq_header("[Header]", "Header");
        assert_eq_header(" [  Whitespaced\tHeader  ]\n\n", "Whitespaced\tHeader");
        assert_eq_header("[Header = with = equals]", "Header = with = equals");
        assert_eq_header("[#Pound sign header]", "#Pound sign header");
        assert_eq_header("[]", "");
        assert_eq!(
            err_string("[Bad header"),
            "no closing bracket \"]\" in header line: \"[Bad header\""
        );
    }


    #[test]
    fn test_definition() {
        fn assert_eq_vardef(line: &str, expected_var: &str, expected_def: &str) {
            let input_line = line.parse::<InputLine>().unwrap();
            if let Some(definition) = input_line.definition() {
                assert_eq!(definition, (expected_var, expected_def));
            } else {
                panic!("not a definition: {}", line.to_owned());
            }
            assert_eq!(input_line.kind(), InputLineKind::Definition);
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
        assert_eq!(
            err_string("=#def"),
            "no variable name before \"=\" of a variable definition: \"=#def\""
        );
        assert_eq!(
            err_string("="),
            "no variable name before \"=\" of a variable definition: \"=\""
        );
        assert_eq!(
            err_string("var!"),
            "no equals sign \"=\" in variable definition: \"var!\""
        );
    }


    #[test]
    fn test_comment() {
        fn assert_eq_comment(line: &str) {
            let input_line = line.parse::<InputLine>().unwrap();
            assert!(input_line.is_comment());
            assert_eq!(input_line.kind(), InputLineKind::Comment);
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
