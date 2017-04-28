
use std::fs::File;
use std::error::Error;
use std::fmt::{self, Display};
use std::io::{self, BufRead};

use super::scenario::{Scenario, ScenarioError};
use super::inputline::{InputLine, SyntaxError};


pub fn are_names_unique<'a, I>(scenarios: I) -> bool
where
    I: 'a + IntoIterator<Item = &'a Scenario>,
{
    let mut names = ::std::collections::HashSet::new();
    scenarios.into_iter().all(|s| names.insert(s.name()))
}


/// Opens a file and reads scenarios from it.
///
/// If an error occurs, it contains the path of the offending file.
pub fn from_file<S: Into<String>>(path: S) -> Result<Vec<Scenario>, FileParseError> {
    let path = path.into();
    match File::open(&path) {
        Ok(file) => from_named_buffer(io::BufReader::new(file), path),
        Err(err) => Err(FileParseError::new(err, path)),
    }
}

/// Reads scenarios from a given buffered reader.
///
/// If an error occurs, it is enriched with the given name.
pub fn from_named_buffer<F, S>(buffer: F, name: S) -> Result<Vec<Scenario>, FileParseError>
where
    F: BufRead,
    S: Into<String>,
{
    from_buffer(buffer).map_err(|err| err.add_filename(name))
}

/// Reads scenarios from a buffered reader.
pub fn from_buffer<F: BufRead>(buffer: F) -> Result<Vec<Scenario>, LineParseError> {
    ScenariosIter::new(buffer)?.collect()
}


/// The iterator returned by `Scenario::iter_from_file()`.
#[derive(Debug)]
pub struct ScenariosIter<F: BufRead> {
    /// The wrapped iterator of input file lines.
    lines: io::Lines<F>,
    /// Intermediate buffer for the next scenario's name.
    next_header: Option<String>,
    /// The current input line number, used for error messages.
    current_lineno: usize,
}


impl<F: BufRead> ScenariosIter<F> {
    /// Creates a new instance.
    ///
    /// This takes a `BufRead` instance and drops lines until the
    /// first header line has been found.
    ///
    /// # Errors
    /// See `scan_to_first_header()` for a description of error modes.
    fn new(file: F) -> Result<Self, LineParseError> {
        let mut result = ScenariosIter {
            lines: file.lines(),
            next_header: None,
            current_lineno: 0,
        };
        if let Err(err) = result.skip_to_next_header() {
            return Err(err.add_lineno(result.current_lineno));
        }
        Ok(result)
    }

    /// Drop lines until the next header line appears.
    ///
    /// This sets `self.next_header` to the found header line. If no
    /// further header line is found, it is set to `None`. No variable
    /// definitions may occur. This should only be called from within
    /// `new()`.
    ///
    /// # Errors
    /// * `ParseError::IoError` if a line cannot be read.
    /// * `ParseError::SyntaxError` if a line fails to be parsed.
    /// * `ParseError::UnexpectedVarDef` if a variable definition is
    ///   found. Since no scenario has been declared yet, any
    ///   definition would be out of place.
    fn skip_to_next_header(&mut self) -> Result<(), ParseError> {
        // Set it to `None` first, in case of error. If we actually do
        // find a header, we can set it to `Some` again.
        self.next_header = None;
        while let Some(line) = self.next_line() {
            match line?.parse::<InputLine>()? {
                InputLine::Comment => {},
                InputLine::Header(header) => {
                    self.next_header = Some(header);
                    return Ok(());
                },
                InputLine::Definition(varname, _) => {
                    return Err(ParseError::UnexpectedVardef(varname).into());
                },
            }
        }
        // No further header found, `next_header` stays `None`.
        Ok(())
    }

    /// Continue parsing the file until the next header line or EOF.
    ///
    /// For simplicity's sake, this function is *passed* the previous
    /// sections header line instead of taking it itself. It returns
    /// the scenario belonging to this header line.
    ///
    /// This function is private and merely a convenience helper for
    /// `<Iter<F> as Iterator>::next()`.
    ///
    /// # Errors
    ///
    /// `ParseError::SyntaxError` if a line fails to be parsed as
    /// header, definition, or comment line.
    fn read_next_section(&mut self) -> Result<Option<Scenario>, ParseError> {
        // Calling take ensures that any error immediately exhausts the
        // entire iterator by leaving `None` in `next_header`.
        let mut result = match self.next_header.take() {
            Some(header) => Scenario::new(header)?,
            None => return Ok(None),
        };
        while let Some(line) = self.next_line() {
            match line?.parse::<InputLine>()? {
                InputLine::Comment => {},
                InputLine::Header(name) => {
                    self.next_header = Some(name);
                    break;
                },
                InputLine::Definition(name, value) => {
                    result.add_variable(name, value)?;
                },
            }
        }
        Ok(Some(result))
    }

    /// Fetches the next line and increments the current line counter.
    fn next_line(&mut self) -> Option<io::Result<String>> {
        self.current_lineno += 1;
        self.lines.next()
    }
}

impl<F: BufRead> Iterator for ScenariosIter<F> {
    type Item = Result<Scenario, LineParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_next_section() {
            Ok(Some(result)) => Some(Ok(result)),
            Ok(None) => None,
            Err(err) => Some(Err(err.add_lineno(self.current_lineno))),
        }
    }
}


/// An error that occured while handling a specific file.
#[derive(Debug)]
pub struct FileParseError {
    kind: FileParseErrorKind,
    filename: String,
}

impl FileParseError {
    /// Create a new error.
    ///
    /// The `inner` argument should be either an `std::io::Error` or
    /// a `LineParseError`.
    fn new<E, S>(inner: E, filename: S) -> Self
    where
        E: Into<FileParseErrorKind>,
        S: Into<String>,
    {
        FileParseError {
            kind: inner.into(),
            filename: filename.into(),
        }
    }
}

impl Display for FileParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: ", self.filename)?;
        match self.kind {
            FileParseErrorKind::LineParseError(ref err) => err.fmt(f),
            FileParseErrorKind::IoError(ref err) => err.fmt(f),
        }
    }
}

impl Error for FileParseError {
    fn description(&self) -> &str {
        match self.kind {
            FileParseErrorKind::LineParseError(ref err) => err.description(),
            FileParseErrorKind::IoError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self.kind {
            FileParseErrorKind::LineParseError(ref err) => Some(err),
            FileParseErrorKind::IoError(ref err) => Some(err),
        }
    }
}


/// Enum of the different kinds of errors wrapped by `FileParseError`.
#[derive(Debug)]
enum FileParseErrorKind {
    LineParseError(LineParseError),
    IoError(io::Error),
}

impl From<LineParseError> for FileParseErrorKind {
    fn from(err: LineParseError) -> Self {
        FileParseErrorKind::LineParseError(err)
    }
}

impl From<io::Error> for FileParseErrorKind {
    fn from(err: io::Error) -> Self {
        FileParseErrorKind::IoError(err)
    }
}


/// A `ParseError` with additional line number information.
#[derive(Debug)]
pub struct LineParseError {
    inner: ParseError,
    lineno: usize,
}

impl LineParseError {
    /// Enrich this error with the file in which it occurred.
    fn add_filename<S: Into<String>>(self, filename: S) -> FileParseError {
        FileParseError::new(self, filename)
    }
}

impl Display for LineParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "in line {}: ", self.lineno)?;
        self.inner.fmt(f)
    }
}

impl Error for LineParseError {
    fn description(&self) -> &str {
        self.inner.description()
    }

    fn cause(&self) -> Option<&Error> {
        Some(&self.inner)
    }
}


/// Any type of error that occurs during parsing of scenario files.
#[derive(Debug)]
pub enum ParseError {
    IoError(io::Error),
    SyntaxError(SyntaxError),
    ScenarioError(ScenarioError),
    UnexpectedVardef(String),
}

impl ParseError {
    /// Enrich this error with the line in which it occurred.
    fn add_lineno(self, lineno: usize) -> LineParseError {
        LineParseError {
            inner: self,
            lineno: lineno,
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseError::IoError(ref err) => err.fmt(f),
            ParseError::SyntaxError(ref err) => err.fmt(f),
            ParseError::ScenarioError(ref err) => err.fmt(f),
            ParseError::UnexpectedVardef(ref s) => write!(f, "{}: {}", self.description(), s),
        }
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::IoError(ref err) => err.description(),
            ParseError::SyntaxError(ref err) => err.description(),
            ParseError::ScenarioError(ref err) => err.description(),
            ParseError::UnexpectedVardef(_) => "variable definition before the first header",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ParseError::IoError(ref err) => Some(err),
            ParseError::SyntaxError(ref err) => Some(err),
            ParseError::ScenarioError(ref err) => Some(err),
            ParseError::UnexpectedVardef(_) => None,
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> Self {
        ParseError::IoError(err)
    }
}

impl From<SyntaxError> for ParseError {
    fn from(err: SyntaxError) -> Self {
        ParseError::SyntaxError(err)
    }
}

impl From<ScenarioError> for ParseError {
    fn from(err: ScenarioError) -> Self {
        ParseError::ScenarioError(err)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::io::Cursor;


    fn get_scenarios(contents: &str) -> Vec<Scenario> {
        from_buffer(Cursor::new(contents)).unwrap()
    }

    fn assert_vars(s: &Scenario, variables: &[(&str, &str)]) {
        let expected_names: HashSet<&str> = variables.iter().map(|&(name, _)| name).collect();
        let actual_names: HashSet<&str> = s.variable_names().map(String::as_str).collect();
        assert_eq!(expected_names, actual_names);

        for &(name, value) in variables {
            assert_eq!(Some(value), s.get_variable(name));
        }
    }


    #[test]
    fn test_iter_from_file() {
        let output = get_scenarios(
            "\
            [First Scenario]
            aaaa = 1
            bbbb = 8
            cdcd = complicated value

            [Second Scenario]
            # Comment line
            aaaa=8
            bbbb             =1
            cdcd= lesscomplicated

            [Third Scenario]
            ",
        );
        let mut output = output.iter();

        let the_scenario = output.next().unwrap();
        assert_eq!(the_scenario.name(), "First Scenario");
        let the_variables = [("aaaa", "1"), ("bbbb", "8"), ("cdcd", "complicated value")];
        assert_vars(the_scenario, &the_variables);

        let the_scenario = output.next().unwrap();
        assert_eq!(the_scenario.name(), "Second Scenario");
        let the_variables = [("aaaa", "8"), ("bbbb", "1"), ("cdcd", "lesscomplicated")];
        assert_vars(the_scenario, &the_variables);

        let the_scenario = output.next().unwrap();
        assert_eq!(the_scenario.name(), "Third Scenario");
        assert_vars(the_scenario, &[]);

        assert!(output.next().is_none());
    }


    #[test]
    fn test_are_names_unique() {
        let output = get_scenarios(
            "\
            [first]
            [second]
            [third]
            ",
        );
        assert!(are_names_unique(&output));

        let output = get_scenarios(
            "\
            [first]
            [second]
            [third]
            [second]
            ",
        );
        assert!(!are_names_unique(&output));

    }
}
