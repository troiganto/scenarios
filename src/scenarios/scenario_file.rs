
use std::fs::File;
use std::error::Error;
use std::fmt::{self, Display};
use std::io::{self, BufRead};
use std::borrow::{Borrow, ToOwned};

use quick_error::{Context, ResultExt};

use super::location::ErrorLocation;
use super::scenario::{Scenario, ScenarioError};
use super::inputline::{InputLine, SyntaxError};


/// Returns `false` if two of the given `scenarios` have the same name.
pub fn are_names_unique<'a, I>(scenarios: I) -> bool
where
    I: 'a + IntoIterator<Item = &'a Scenario>,
{
    let mut names = ::std::collections::HashSet::new();
    scenarios.into_iter().all(|s| names.insert(s.name()))
}


/// Like `from_file`, but also handles `path == "-"`.
///
/// If `path` equals `"-"`, this reads scenarios from stdin. Otherwise,
/// it treats `path` like a regular file path and calls `from_file`.
pub fn from_file_or_stdin<S: Borrow<str>>(path: S) -> Result<Vec<Scenario>, ParseError> {
    let stdin = io::stdin();
    if path.borrow() == "-" {
        from_named_buffer(stdin.lock(), "<stdin>")
    } else {
        from_file(path)
    }
}

/// Opens a file and reads scenarios from it.
///
/// If an error occurs, the error contains the path of the offending
/// file.
pub fn from_file<S: Borrow<str>>(path: S) -> Result<Vec<Scenario>, ParseError> {
    let path = path.borrow();
    match File::open(path) {
        Ok(file) => from_named_buffer(io::BufReader::new(file), path),
        Err(err) => Err(ParseError(ErrorLocation::new(path.to_owned()), err.into())),
    }
}

/// Reads scenarios from a given buffered reader.
pub fn from_named_buffer<F, S>(buffer: F, name: S) -> Result<Vec<Scenario>, ParseError>
where
    F: BufRead,
    S: Borrow<str>,
{
    ScenariosIter::new(buffer, name.borrow()).and_then(Iterator::collect)
}


/// An iterator that reads `Scenario`s from a `BufRead` variable.
#[derive(Debug)]
struct ScenariosIter<'a, F: BufRead> {
    /// The wrapped iterator of input file lines.
    lines: io::Lines<F>,
    /// Intermediate buffer for the next scenario's name.
    next_header: Option<String>,
    /// The current filename and line number, used for error messages.
    location: ErrorLocation<&'a str>,
}


impl<'a, F: BufRead> ScenariosIter<'a, F> {
    /// Creates a new instance.
    ///
    /// This takes a `BufRead` instance and drops lines until the
    /// first header line has been found.
    ///
    /// The `filename` is used only for error messages.
    ///
    /// # Errors
    /// See `scan_to_first_header()` for a description of error modes.
    fn new(file: F, filename: &'a str) -> Result<Self, ParseError> {
        let mut result = ScenariosIter {
            lines: file.lines(),
            next_header: None,
            location: ErrorLocation::new(filename),
        };
        result.skip_to_next_header().context(result.location)?;
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
    /// * `io::Error` if a line cannot be read.
    /// * `inputline::SyntaxError` if a line cannot be interpreted.
    /// * `UnexpectedVarDef` if a variable definition is found. Since
    ///   no scenario has been declared yet, any definition would be
    ///   out of place.
    fn skip_to_next_header(&mut self) -> Result<(), ErrorKind> {
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
                    return Err(ErrorKind::UnexpectedVardef(varname));
                },
            }
        }
        // No further header found, `next_header` stays `None`.
        Ok(())
    }

    /// Continue parsing the file until the next header line or EOF.
    ///
    /// This function returns the scenario belonging to the current
    /// header line. It is private and merely a convenience helper for
    /// `next()`.
    ///
    /// # Errors
    /// * `io::Error` if a line cannot be read.
    /// * `inputline::SyntaxError` if a line cannot be interpreted.
    fn read_next_section(&mut self) -> Result<Option<Scenario>, ErrorKind> {
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
        self.location.lineno += 1;
        self.lines.next()
    }
}

impl<'a, F: BufRead> Iterator for ScenariosIter<'a, F> {
    type Item = Result<Scenario, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_next_section() {
            Ok(Some(result)) => Some(Ok(result)),
            Ok(None) => None,
            Err(err) => Some(Err(ParseError(self.location.to_owned(), err))),
        }
    }
}


/// An error that occured while handling a specific file.
///
/// It is typically created by taking an `ErrorKind` and supplying it
/// with some `quick_error::Context`.
#[derive(Debug)]
pub struct ParseError(ErrorLocation<String>, ErrorKind);

impl ParseError {
    /// Returns the name of the file in which the error occurred.
    pub fn filename(&self) -> &str {
        &self.0.filename
    }

    /// Returns the error's line number, if any.
    ///
    /// If the error is not associated with a particular line, this
    /// returns `None`. Otherwise, it returns the line number, starting
    /// at `1` for the first line.
    ///
    /// In short, this never returns `Some(0)`.
    pub fn lineno(&self) -> Option<usize> {
        if self.0.lineno != 0 {
            Some(self.0.lineno)
        } else {
            None
        }
    }

    /// Returns the kind of error that happened.
    pub fn kind(&self) -> &ErrorKind {
        &self.1
    }
}

impl<'a, S, E> From<Context<ErrorLocation<&'a S>, E>> for ParseError
where
    String: Borrow<S>,
    S: ToOwned<Owned=String> + ?Sized,
    E: Into<ErrorKind>,
{
    fn from(context: Context<ErrorLocation<&'a S>, E>) -> Self {
        ParseError(context.0.to_owned(), context.1.into())
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.0, self.1)
    }
}

impl Error for ParseError {
    fn description(&self) -> &str {
        self.1.description()
    }

    fn cause(&self) -> Option<&Error> {
        self.1.cause()
    }
}


quick_error! {
    /// Any type of error that occurs during parsing of scenario files.
    #[derive(Debug)]
    pub enum ErrorKind {
        IoError(err: io::Error) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        SyntaxError(err: SyntaxError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        ScenarioError(err: ScenarioError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        UnexpectedVardef(name: String) {
            description("variable definition before the first header")
            display(err) -> ("{}: \"{}\"", err.description(), name)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::io::Cursor;


    fn get_scenarios(contents: &str) -> Result<Vec<Scenario>, ParseError> {
        from_named_buffer(Cursor::new(contents), "<memory>")
    }

    fn assert_vars(s: &Scenario, variables: &[(&str, &str)]) {
        // Check first the names for equality.
        let expected_names = variables
            .iter()
            .map(|&(name, _)| name)
            .collect::<HashSet<_>>();
        let actual_names = s.variable_names()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        assert_eq!(expected_names, actual_names);
        // Then check that the values are equal, too.
        for &(name, value) in variables {
            assert_eq!(Some(value), s.get_variable(name));
        }
    }


    #[test]
    fn test_iter_from_file() {
        let file = r"
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
            ";
        let output = get_scenarios(file).expect("parse failed");
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
    fn test_errors() {
        let file = "[scenario]\nthe bad line";
        assert_eq!(
            get_scenarios(file)
                .expect_err("no syntax error found")
                .to_string(),
            "<memory>:2: could not parse line: \"the bad line\""
        );
        let file = r"[scenario]
        varname = value
        varname = other value
        ";
        assert_eq!(
            get_scenarios(file)
                .expect_err("no duplicate definition found")
                .to_string(),
            "<memory>:3: variable already defined: \"varname\""
        );
        let file = "[scenario]\n[key] = value";
        assert_eq!(
            get_scenarios(file)
                .expect_err("no invalid variable name found")
                .to_string(),
            "<memory>:2: invalid variable name: \"[key]\""
        );
        let file = r"[scenario]
        a = b
        []
        ";
        assert_eq!(
            get_scenarios(file)
                .expect_err("no invalid scenario name found")
                .to_string(),
            "<memory>:3: invalid scenario name: \"\""
        );
        let file = r"
        # second line
        # third line

        # fifth line
        a = b
        ";
        assert_eq!(
            get_scenarios(file)
                .expect_err("no unexpected variable definition found")
                .to_string(),
            "<memory>:6: variable definition before the first header: \"a\""
        );
    }

    #[test]
    fn test_are_names_unique() {
        let file = r"
            [first]
            [second]
            [third]
            ";
        let output = get_scenarios(file).expect("parse of unique names failed");
        assert!(are_names_unique(&output));

        let file = r"
            [first]
            [second]
            [third]
            [second]
            ";
        let output = get_scenarios(file).expect("parse of non-unique names failed");
        assert!(!are_names_unique(&output));

    }
}
