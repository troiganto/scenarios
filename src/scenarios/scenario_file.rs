
use std::fs::File;
use std::error::Error;
use std::fmt::{self, Display};
use std::io::{self, BufRead};
use std::borrow::{Borrow, ToOwned};

use quick_error::{Context, ResultExt};

use super::location::ErrorLocation;
use super::scenario::{Scenario, ScenarioError};
use super::inputline::{InputLine, SyntaxError};


#[derive(Debug)]
pub struct ScenarioFile<'a> {
    filename: &'a str,
    lines: Vec<InputLine>,
}

impl<'a> ScenarioFile<'a> {
    /// Like `from_file`, but also handles `path == "-"`.
    ///
    /// If `path` equals `"-"`, this reads scenarios from stdin.
    /// Otherwise,
    /// it treats `path` like a regular file path and calls `from_file`.
    pub fn from_file_or_stdin(path: &str) -> Result<ScenarioFile, ParseError> {
        let stdin = io::stdin();
        if path.borrow() == "-" {
            Self::new(stdin.lock(), "<stdin>")
        } else {
            Self::from_file(path)
        }
    }

    /// Opens a file and reads scenarios from it.
    ///
    /// If an error occurs, the error contains the path of the offending
    /// file.
    pub fn from_file(path: &str) -> Result<ScenarioFile, ParseError> {
        let path = path.borrow();
        let file = File::open(path).context(ErrorLocation::new(path))?;
        let file = io::BufReader::new(file);
        Self::new(file, path)
    }

    /// Reads scenarios from a given buffered reader.
    pub fn new<F: BufRead>(reader: F, filename: &str) -> Result<ScenarioFile, ParseError> {
        let mut loc = ErrorLocation::new(filename);
        let lines = Self::new_impl(reader, &mut loc).context(loc)?;
        Ok(ScenarioFile { filename, lines })
    }

    /// The actual implementation of `new()`.
    ///
    /// In the case of an error, the `loc` parameter is used to report
    /// back the line in which the error occurred.
    fn new_impl<F: BufRead>(
        mut reader: F,
        loc: &mut ErrorLocation<&'a str>,
    ) -> Result<Vec<InputLine>, ErrorKind> {
        let mut lines = Vec::new();
        let mut buffer = String::new();
        loop {
            loc.lineno += 1;
            let num_bytes = reader.read_line(&mut buffer)?;
            if num_bytes == 0 {
                return Ok(lines);
            }
            lines.push(buffer.parse::<InputLine>()?);
            buffer.clear();
        }
    }

    pub fn filename(&self) -> &str {
        self.filename
    }

    pub fn iter(&self) -> ScenariosIter {
        ScenariosIter::new(self.filename, &self.lines)
    }
}

impl<'a, 'b: 'a> IntoIterator for &'a ScenarioFile<'b> {
    type IntoIter = ScenariosIter<'a>;
    type Item = <Self::IntoIter as Iterator>::Item;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}


/// An iterator that reads `Scenario`s from a `BufRead` variable.
#[derive(Debug, Clone)]
pub struct ScenariosIter<'a> {
    location: ErrorLocation<&'a str>,
    lines: &'a [InputLine],
}

impl<'a> ScenariosIter<'a> {
    /// Creates a new instance.
    fn new(filename: &'a str, lines: &'a [InputLine]) -> Self {
        let location = ErrorLocation::new(filename);
        ScenariosIter { location, lines }
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
    fn next_scenario(&mut self) -> Result<Option<Scenario<'a>>, ErrorKind> {
        let mut scenario = match self.next_header_line()? {
            Some(line) => Scenario::new(line)?,
            None => return Ok(None),
        };
        while let Some((name, value)) = self.next_definition_line() {
            scenario.add_variable(name, value)?;
        }
        Ok(Some(scenario))
    }

    /// Fetches the next header line, skipping over comments.
    ///
    /// # Errors
    /// If a definition line is found, the line counter is still
    /// incremented, but a `ScenarioError::UnexpectedVarDef` is
    /// returned.
    fn next_header_line(&mut self) -> Result<Option<&'a str>, ErrorKind> {
        while let Some(line) = self.lines.get(self.location.lineno) {
            self.location.lineno += 1;
            if let Some(header) = line.header() {
                return Ok(Some(header));
            } else if let Some(name) = line.definition_name() {
                return Err(ErrorKind::UnexpectedVarDef(name.into()));
            }
        }
        Ok(None)
    }

    /// Fetches the next definition line.
    ///
    /// Comment lines are skipped over. If a header line is
    /// encountered, `None` is returned and the line counter is *not*
    /// incremented. In other words, calling `next_line()` after this
    /// method will give eitehr `None` or `Some(InputLine::Header)`.
    fn next_definition_line(&mut self) -> Option<(&'a str, &'a str)> {
        while let Some(line) = self.lines.get(self.location.lineno) {
            if line.is_header() {
                // Leave *without* moving to the next line.
                break;
            } else {
                self.location.lineno += 1;
                if let Some(parts) = line.definition() {
                    return Some(parts);
                }
            }
        }
        None
    }
}

impl<'a> Iterator for ScenariosIter<'a> {
    type Item = Result<Scenario<'a>, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_scenario().context(self.location) {
            Ok(None) => None,
            Ok(Some(scenario)) => Some(Ok(scenario)),
            Err(context) => Some(Err(ParseError::from(context))),
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
    /// Creates a new `ParseError`, performing coercions as necessary.
    ///
    /// `loc` is a `ErrorLocation` borrowing or owning its `filename`.
    /// `kind` is any error that can be converted to `ErrorKind`.
    fn new<'a, S, E>(loc: ErrorLocation<&'a S>, kind: E) -> Self
    where
        String: Borrow<S>,
        S: ToOwned<Owned = String> + ?Sized,
        E: Into<ErrorKind>,
    {
        ParseError(loc.to_owned(), kind.into())
    }

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

/// Build a `ParseError` from an `ErrorKind` in a `Context`.
///
/// This uses the *context* mechanism of `quick_error`. Given a value
/// of type `Result<_, ErrorKind>`, we can supply it with a `Context`.
/// This context is an `ErrorLocation`, i.e. a file name and line
/// number. These can be put together in an automatic way to build a
/// proper `ParseError`.
///
/// # Example
///
/// ```rust
/// use scenarios::location::ErrorLocation;
/// use scenarios::scenario_file::{ParseError, ErrorKind};
/// use quick_error::ResultExt;
///
/// let lines = vec!["a", "b", "c"];
/// let err = returns_parse_error(lines).unwrap_err();
/// assert_eq!(err.lineno(), Some(2));
///
/// fn returns_parse_error<I>(lines: I) -> Result<(), ParseError>
/// where
///     I: Iterator<Item = str>
/// {
///     let location = ErrorLocation::new("file");
///     for line in lines {
///         location.lineno += 1;
///         fails_on_b(&line).context(location)?;
///     }
/// }
///
/// fn fails_on_b(line: &str) -> Result<(), ErrorKind> {
///     if line != "b" {
///         Ok(())
///     } else {
///         Err(ErrorKind::UnexpectedVardef(line.into())
///     }
/// }
/// ```
impl<'a, S, E> From<Context<ErrorLocation<&'a S>, E>> for ParseError
where
    String: Borrow<S>,
    S: ToOwned<Owned=String> + ?Sized,
    E: Into<ErrorKind>,
{
    fn from(context: Context<ErrorLocation<&'a S>, E>) -> Self {
        ParseError::new(context.0, context.1)
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


// TODO: Add this test to ScenarioFile::new()! Add a unit test for that!
/// Returns `false` if two of the given `scenarios` have the same name.
fn are_names_unique<'a, I>(scenarios: I) -> bool
where
    I: 'a + IntoIterator<Item = &'a Scenario<'a>>,
{
    let mut names = ::std::collections::HashSet::new();
    scenarios.into_iter().all(|s| names.insert(s.name()))
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
        UnexpectedVarDef(name: String) {
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


    fn get_scenarios(contents: &str) -> Result<ScenarioFile, ParseError> {
        ScenarioFile::new(Cursor::new(contents), "<memory>")
    }

    fn assert_vars(s: &Scenario, variables: &[(&str, &str)]) {
        // Check first the names for equality.
        let expected_names = variables
            .iter()
            .map(|&(name, _)| name)
            .collect::<HashSet<_>>();
        let actual_names = s.variable_names().cloned().collect::<HashSet<_>>();
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
        let output = get_scenarios(file).unwrap();
        let mut output = output.iter();

        let the_scenario = output
            .next()
            .expect("no scenario")
            .expect("scenario error");
        assert_eq!(the_scenario.name(), "First Scenario");
        let the_variables = [("aaaa", "1"), ("bbbb", "8"), ("cdcd", "complicated value")];
        assert_vars(&the_scenario, &the_variables);

        let the_scenario = output
            .next()
            .expect("no scenario")
            .expect("scenario error");
        assert_eq!(the_scenario.name(), "Second Scenario");
        let the_variables = [("aaaa", "8"), ("bbbb", "1"), ("cdcd", "lesscomplicated")];
        assert_vars(&the_scenario, &the_variables);

        let the_scenario = output
            .next()
            .expect("no scenario")
            .expect("scenario error");
        assert_eq!(the_scenario.name(), "Third Scenario");
        assert_vars(&the_scenario, &[]);

        assert!(output.next().is_none());
    }

    #[test]
    fn test_unique_names() {
        let file = get_scenarios("[first]\n[second]\n[third]\n").unwrap();
        let scenarios = file.iter().collect::<Result<Vec<_>, _>>().unwrap();
        assert!(are_names_unique(scenarios.iter()));
    }

    #[test]
    fn test_non_unique_names() {
        let file = get_scenarios("[first]\n[second]\n[third]\n[second]").unwrap();
        let scenarios = file.iter().collect::<Result<Vec<_>, _>>().unwrap();
        assert!(!are_names_unique(scenarios.iter()));

    }

    #[test]
    fn test_invalid_variable_def() {
        let expected_message = "<memory>:2: syntax error: missing equals sign \"=\" in variable \
                                definition: \"the bad line\"";
        let file = "[scenario]\nthe bad line";
        assert_eq!(
            get_scenarios(file).unwrap_err().to_string(),
            expected_message
        );
    }

    #[test]
    fn test_variable_already_defined() {
        let expected_message = "<memory>:3: variable already defined: \"varname\"";
        let file = r"[scenario]
        varname = value
        varname = other value
        ";
        let file = get_scenarios(file).unwrap();
        let scenarios = file.iter().collect::<Result<Vec<_>, _>>();
        assert_eq!(scenarios.unwrap_err().to_string(), expected_message);
    }

    #[test]
    fn test_invalid_header() {
        let expected_message = "<memory>:2: syntax error: text after closing bracket \"]\" of a \
                                header line: \"[key] = value\"";
        let file = get_scenarios("[scenario]\n[key] = value");
        assert_eq!(file.unwrap_err().to_string(), expected_message);
    }

    #[test]
    fn test_invalid_variable_name() {
        let expected_message = "<memory>:2: invalid variable name: \"ß\"";
        let file = get_scenarios("[scenario]\nß = ss").unwrap();
        let scenarios = file.iter().collect::<Result<Vec<_>, _>>();
        assert_eq!(scenarios.unwrap_err().to_string(), expected_message);
    }

    #[test]
    fn test_invalid_scenario_name() {
        let expected_message = "<memory>:3: invalid scenario name: \"\"";
        let file = get_scenarios("[scenario]\na = b\n[]\n").unwrap();
        let scenarios = file.iter().collect::<Result<Vec<_>, _>>();
        assert_eq!(scenarios.unwrap_err().to_string(), expected_message);
    }

    #[test]
    fn test_unexpected_vardef() {
        let expected_message = "<memory>:6: variable definition before the first header: \"a\"";
        let file = r"
        # second line
        # third line

        # fifth line
        a = b
        ";
        let file = get_scenarios(file).unwrap();
        let scenarios = file.iter().collect::<Result<Vec<_>, _>>();
        assert_eq!(scenarios.unwrap_err().to_string(), expected_message);
    }
}
