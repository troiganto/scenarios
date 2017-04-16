
use std::fs::File;
use std::error::Error;
use std::fmt::{self, Display};
use std::io::{self, BufRead};

use scenario::{Scenario, ScenarioError};
use inputline::{InputLine, SyntaxError};


/// Type alias for convenience.
type Result<T> = ::std::result::Result<T, ParseError>;


pub fn are_names_unique<'a, I>(scenarios: I) -> bool
    where I: 'a + IntoIterator<Item = &'a Scenario>
{
    let mut names = ::std::collections::HashSet::new();
    scenarios.into_iter().all(|s| names.insert(s.name()))
}


/// Opens a file and reads scenarios from it.
///
/// If an error occurs, it contains the path of the offending file.
pub fn scenarios_from_file<S: Into<String>>(path: S) -> Result<Vec<Scenario>> {
    let path = path.into();
    let file = File::open(&path)?;
    scenarios_from_named_buffer(io::BufReader::new(file), path)
}

/// Reads scenarios from a given buffered reader.
///
/// If an error occurs, it is enriched with the given name.
pub fn scenarios_from_named_buffer<F, S>(buffer: F, name: S) -> Result<Vec<Scenario>>
    where F: BufRead,
          S: Into<String>
{
    let mut result = scenarios_from_buffer(buffer);
    if let Err(ref mut err) = result.as_mut() {
        err.set_filename(name.into());
    }
    result
}

/// Reads scenarios from a buffered reader.
pub fn scenarios_from_buffer<F: BufRead>(buffer: F) -> Result<Vec<Scenario>> {
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
    fn new(file: F) -> Result<Self> {
        let mut result = ScenariosIter {
            lines: file.lines(),
            next_header: None,
            current_lineno: 0,
        };
        result.skip_to_next_header()?;
        Ok(result)
    }

    /// Drop lines in the input iterator until the next header line appears.
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
    fn skip_to_next_header(&mut self) -> Result<()> {
        // Set it to `None` first, in case of error. If we actually do
        // find a header, we can set it to `Some` again.
        self.next_header = None;
        while let Some(line) = self.next_line() {
            match line?.parse::<InputLine>()? {
                InputLine::Comment => {}
                InputLine::Header(header) => {
                    self.next_header = Some(header);
                    return Ok(());
                }
                InputLine::Definition(varname, _) => {
                    return Err(ErrorKind::UnexpectedVardef(varname).into());
                }
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
    fn read_next_section(&mut self) -> Result<Option<Scenario>> {
        // Calling take ensures that any error immediately exhausts the
        // entire iterator by leaving `None` in `next_header`.
        let mut result = match self.next_header.take() {
            Some(header) => Scenario::new(header)?,
            None => return Ok(None),
        };
        while let Some(line) = self.next_line() {
            match line?.parse::<InputLine>()? {
                InputLine::Comment => {}
                InputLine::Header(name) => {
                    self.next_header = Some(name);
                    break;
                }
                InputLine::Definition(name, value) => {
                    result.add_variable(name, value)?;
                }
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
    type Item = Result<Scenario>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_next_section() {
            Ok(Some(result)) => Some(Ok(result)),
            Ok(None) => None,
            Err(mut err) => {
                err.set_lineno(self.current_lineno);
                Some(Err(err))
            }
        }
    }
}


/// Error that combines all errors that can happen during file parsing.
///
/// This error type allows being "enriched" with file name and line
/// number information for more detailed error messages.
#[derive(Debug)]
pub struct ParseError {
    /// The specific kind of error.
    kind: ErrorKind,
    lineno: Option<usize>,
    filename: Option<String>,
}

impl ParseError {
    /// Creates a new error wrapping the given error kind.
    fn new(kind: ErrorKind) -> Self {
        ParseError {
            kind: kind,
            lineno: None,
            filename: None,
        }
    }

    /// Gets the number of the offending line.
    fn lineno(&self) -> Option<usize> {
        self.lineno
    }

    /// Sets the number of the offending line.
    fn set_lineno(&mut self, lineno: usize) {
        self.lineno = Some(lineno);
    }

    /// Gets the name of the file containing the offending line.
    fn filename(&self) -> Option<&str> {
        self.filename.as_ref().map(String::as_str)
    }

    /// Sets the name of the file containing the offending line.
    fn set_filename<S: Into<String>>(&mut self, filename: S) {
        self.filename = Some(filename.into());
    }

    /// Returns the kind of error wrapped by this struct.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Converts the error into the wrapped error kind.
    pub fn into_kind(self) -> ErrorKind {
        self.kind
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (self.lineno, self.filename.as_ref()) {
            (Some(lineno), Some(name)) => {
                write!(f, "{}:{}: ", name, lineno)?;
            }
            (Some(lineno), None) => {
                write!(f, "line {}: ", lineno)?;
            }
            (None, Some(name)) => {
                write!(f, "{}: ", name)?;
            }
            (None, None) => {}
        }
        self.kind.fmt(f)
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        self.kind.description()
    }

    fn cause(&self) -> Option<&Error> {
        self.kind.cause()
    }
}

impl<T: Into<ErrorKind>> From<T> for ParseError {
    fn from(err: T) -> Self {
        Self::new(err.into())
    }
}


/// Enum that describes the specific error wrapped by `ParseError`.
#[derive(Debug)]
pub enum ErrorKind {
    IoError(io::Error),
    SyntaxError(SyntaxError),
    ScenarioError(ScenarioError),
    UnexpectedVardef(String),
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorKind::IoError(ref err) => err.fmt(f),
            ErrorKind::SyntaxError(ref err) => err.fmt(f),
            ErrorKind::ScenarioError(ref err) => err.fmt(f),
            ErrorKind::UnexpectedVardef(ref s) => write!(f, "{}: {}", self.description(), s),
        }
    }
}

impl Error for ErrorKind {
    fn description(&self) -> &str {
        match *self {
            ErrorKind::IoError(ref err) => err.description(),
            ErrorKind::SyntaxError(ref err) => err.description(),
            ErrorKind::ScenarioError(ref err) => err.description(),
            ErrorKind::UnexpectedVardef(_) => "variable definition before the first header",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            ErrorKind::IoError(ref err) => Some(err),
            ErrorKind::SyntaxError(ref err) => Some(err),
            ErrorKind::ScenarioError(ref err) => Some(err),
            ErrorKind::UnexpectedVardef(_) => None,
        }
    }
}

impl From<io::Error> for ErrorKind {
    fn from(err: io::Error) -> Self {
        ErrorKind::IoError(err)
    }
}

impl From<SyntaxError> for ErrorKind {
    fn from(err: SyntaxError) -> Self {
        ErrorKind::SyntaxError(err)
    }
}

impl From<ScenarioError> for ErrorKind {
    fn from(err: ScenarioError) -> Self {
        ErrorKind::ScenarioError(err)
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::io::Cursor;


    fn get_scenarios(contents: &str) -> Vec<Scenario> {
        scenarios_from_buffer(Cursor::new(contents)).unwrap()
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

        let output = get_scenarios("\
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
        ");
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
        let output = get_scenarios("\
        [first]
        [second]
        [third]
        ");
        assert!(are_names_unique(&output));

        let output = get_scenarios("\
        [first]
        [second]
        [third]
        [second]
        ");
        assert!(!are_names_unique(&output));

    }
}
