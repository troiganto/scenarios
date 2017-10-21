
use std::fs::File;
use std::error::Error;
use std::fmt::{self, Display};
use std::io::{self, BufRead};

use quick_error::{Context, ResultExt};

use super::scenario::{Scenario, ScenarioError};
use super::inputline::{InputLine, SyntaxError};


// TODO: Document.
pub fn are_names_unique<'a, I>(scenarios: I) -> bool
where
    I: 'a + IntoIterator<Item = &'a Scenario>,
{
    let mut names = ::std::collections::HashSet::new();
    scenarios.into_iter().all(|s| names.insert(s.name()))
}


/// I  `path == "-"`, dispatches to `from_named_buffer`, otherwise to
/// `from_file`.
pub fn from_file_or_stdin<S: Into<String>>(path: S) -> Result<Vec<Scenario>, ParseError> {
    let path = path.into();
    let stdin = io::stdin();
    if path == "-" {
        from_named_buffer(stdin.lock(), "<stdin>")
    } else {
        from_file(path)
    }
}

/// Opens a file and reads scenarios from it.
///
/// If an error occurs, it contains the path of the offending file.
pub fn from_file<S: Into<String>>(path: S) -> Result<Vec<Scenario>, ParseError> {
    let path = path.into();
    match File::open(&path) {
        Ok(file) => from_named_buffer(io::BufReader::new(file), path),
        Err(err) => Err(ParseError(ErrorLocation(path.clone(), 0), err.into())),
    }
}

/// Reads scenarios from a given buffered reader.
///
/// If an error occurs, it is enriched with the given name.
pub fn from_named_buffer<F, S>(buffer: F, name: S) -> Result<Vec<Scenario>, ParseError>
where
    F: BufRead,
    S: Into<String>,
{
    ScenariosIter::new(buffer, name.into()).and_then(Iterator::collect)
}


/// The iterator returned by `Scenario::iter_from_file()`.
#[derive(Debug)]
struct ScenariosIter<F: BufRead> {
    /// The wrapped iterator of input file lines.
    lines: io::Lines<F>,
    /// Intermediate buffer for the next scenario's name.
    next_header: Option<String>,
    /// The current filename and line number, used for error messages.
    location: ErrorLocation,
}


impl<F: BufRead> ScenariosIter<F> {
    /// Creates a new instance.
    ///
    /// This takes a `BufRead` instance and drops lines until the
    /// first header line has been found.
    ///
    /// # Errors
    /// See `scan_to_first_header()` for a description of error modes.
    fn new(file: F, filename: String) -> Result<Self, ParseError> {
        let mut result = ScenariosIter {
            lines: file.lines(),
            next_header: None,
            location: ErrorLocation(filename, 0),
        };
        result.skip_to_next_header()?;
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
            let line = line.context(&self.location)?;
            match line.parse::<InputLine>().context(&self.location)? {
                InputLine::Comment => {},
                InputLine::Header(header) => {
                    self.next_header = Some(header);
                    return Ok(());
                },
                InputLine::Definition(varname, _) => {
                    return Err(
                        ParseError(
                            self.location.clone(),
                            ErrorKind::UnexpectedVardef(varname),
                        ),
                    );
                },
            }
        }
        // No further header found, `next_header` stays `None`.
        Ok(())
    }

    /// Continue parsing the file until the next header line or EOF.
    ///
    /// This function returns the scenario belonging to the current
    /// header line.
    ///
    /// This function is private and merely a convenience helper for
    /// `next()`.
    ///
    /// # Errors
    ///
    /// `ParseError::SyntaxError` if a line fails to be parsed as
    /// header, definition, or comment line.
    fn read_next_section(&mut self) -> Result<Option<Scenario>, ParseError> {
        // Calling take ensures that any error immediately exhausts the
        // entire iterator by leaving `None` in `next_header`.
        // TODO: Reporting wrong location here.
        let mut result = match self.next_header.take() {
            Some(header) => Scenario::new(header).context(&self.location)?,
            None => return Ok(None),
        };
        while let Some(line) = self.next_line() {
            let line = line.context(&self.location)?;
            match line.parse::<InputLine>().context(&self.location)? {
                InputLine::Comment => {},
                InputLine::Header(name) => {
                    self.next_header = Some(name);
                    break;
                },
                InputLine::Definition(name, value) => {
                    result
                        .add_variable(name, value)
                        .context(&self.location)?;
                },
            }
        }
        Ok(Some(result))
    }

    /// Fetches the next line and increments the current line counter.
    fn next_line(&mut self) -> Option<io::Result<String>> {
        self.location.1 += 1;
        self.lines.next()
    }
}

impl<F: BufRead> Iterator for ScenariosIter<F> {
    type Item = Result<Scenario, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_next_section() {
            Ok(Some(result)) => Some(Ok(result)),
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        }
    }
}


// TODO: Document. Make more efficient.
#[derive(Clone, Debug)]
struct ErrorLocation(String, usize);

impl Display for ErrorLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
}


/// An error that occured while handling a specific file.
#[derive(Debug)]
pub struct ParseError(ErrorLocation, ErrorKind);

impl<'a, E: Into<ErrorKind>> From<Context<&'a ErrorLocation, E>> for ParseError {
    fn from(context: Context<&'a ErrorLocation, E>) -> Self {
        ParseError(context.0.clone(), context.1.into())
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
            display(err) -> ("{}: {}", err.description(), name)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::io::Cursor;


    fn get_scenarios(contents: &str) -> Vec<Scenario> {
        from_named_buffer(Cursor::new(contents), "<memory>").unwrap()
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
