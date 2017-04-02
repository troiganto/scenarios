
use std::path::PathBuf;
use std::fs::File;
use std::io::{self, BufRead};
use std::collections::hash_map::{self, HashMap};

use errors::ParseError;
use scenario::Scenario;
use inputline::InputLine;


/// Named collection of scenarios.
///
/// This struct uses `InputLine` to read files and parse them as
/// scenario definitions.
#[derive(Clone, Debug)]
pub struct ScenarioFile {
    path: PathBuf,
    scenarios: Vec<Scenario>
}

impl ScenarioFile {
    /// Create a new named collection of scenarios from a file.
    ///
    /// This reads the input file `file`, which is assumed to be named
    /// `name` and parses it as a list of scenario descriptions.
    ///
    /// # Errors
    /// This call fails if the iterator cannot be constructed. This is
    /// the case if the passed file does not contain any scenarios, if
    /// there is a syntax error before finding the first scenario or if
    /// any I/O error occurs.
    pub fn new<S: Into<PathBuf>>(path: S) -> Result<Self, ParseError> {
        let file = File::open(&path)?;
        ScenarioFile{path: path, scenarios: ScenariosIter::new(file).collect()}
    }

    pub as_slice(&self) -> &[Scenario] { self.scenarios.as_slice() }

    pub iter(&self) -> ::std::vec::Iter { self.scenarios.iter() }
}


/// The iterator returned by `Scenario::iter_from_file()`.
#[derive(Debug)]
pub struct ScenariosIter<F: BufRead> {
    /// The wrapped iterator of input file lines.
    lines: io::Lines<F>,
    /// Intermediate buffer for the next scenario's name.
    next_header: Option<String>,
}


impl<F: BufRead> ScenariosIter<F> {
    /// Creates a new instance.
    ///
    /// This takes a `BufRead` instance and drops lines until the
    /// first header line has been found.
    ///
    /// # Errors
    /// See `scan_to_first_header()` for a description of error modes.
    fn new(file: F) -> Result<Self, ParseError> {
        let mut iter = Iter{lines: file.lines(), next_header: None};
        iter.scan_to_first_header()?;
        Ok(iter)
    }

    /// Finds the first header line and sets `self.next_header`.
    ///
    /// # Errors
    /// * `ParseError::UnexpectedVarDef` if a variable definition is
    ///   found. Since no scenario has been declared yet, any
    ///   definition would be out of place.
    /// * `ParseError::NoScenario` if EOF is reached without finding a
    ///    single header line.
    /// * `ParseError::SyntaxError` if a line fails to be parsed as
    ///    header, definition, or comment line.
    fn scan_to_first_header(&mut self) -> Result<(), ParseError> {
        match InputLine::from_io(&mut self.lines)? {
            InputLine::Header(name) => {
                self.next_header = Some(name);
                Ok(())
            },
            InputLine::Definition(name, _) => {
                Err(ParseError::UnexpectedVarDef(name))
            },
            InputLine::None => {
                Err(ParseError::NoScenario)
            },
            InputLine::SyntaxError(line) => {
                Err(ParseError::SyntaxError(line))
            },
        }
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
    fn read_next_section(&mut self, header: String) -> Result<Scenario, ParseError> {
        let mut result = Scenario::new(header)?;
        loop {
            match InputLine::from_io(&mut self.lines)? {
                InputLine::Definition(name, value) => {
                    result.add_variable(name, value)?;
                },
                InputLine::Header(name) => {
                    self.next_header = Some(name);
                    break;
                },
                InputLine::None => {
                    break;
                },
                InputLine::SyntaxError(line) => {
                    return Err(ParseError::SyntaxError(line));
                },
            }
        }
        Ok(result)
    }
}

impl<F: BufRead> Iterator for ScenariosIter<F> {
    type Item=Result<Scenario, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Take the header line out of `self.next_header` so that
        // `self.next_header` can be filled by `read_next_section()`.
        if let Some(header) = self.next_header.take() {
            let result = self.read_next_section(header);
            Some(result)
        } else {
            None
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_iter_from_file() {
        use std::io::Cursor;

        let input = "\
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
        let file = Cursor::new(input);
        let output = ScenarioFile::new("name", file).unwrap();

        let s = output.iter().next().unwrap();
        assert_eq!(s.name(), "First Scenario");
    }
}
