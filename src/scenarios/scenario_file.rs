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


use std::fs::File;
use std::error::Error;
use std::io::{self, BufRead};
use std::fmt::{self, Display};
use std::collections::hash_map::{HashMap, Entry};

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
    /// Otherwise, it reads from the regular file located at `path`.
    ///
    /// See `new()` for more information.
    pub fn from_file_or_stdin(path: &str, is_strict: bool) -> Result<ScenarioFile, ParseError> {
        let stdin = io::stdin();
        if path == "-" {
            Self::new(stdin.lock(), "<stdin>", is_strict)
        } else {
            let file = File::open(path).context(ErrorLocation::new(path))?;
            let file = io::BufReader::new(file);
            Self::new(file, path, is_strict)
        }
    }

    /// Reads scenarios from a given buffered reader.
    pub fn new<F>(reader: F, filename: &str, is_strict: bool) -> Result<ScenarioFile, ParseError>
    where
        F: BufRead,
    {
        let lines = Vec::new();
        let mut file = ScenarioFile { filename, lines };
        file.read_from(reader)?;
        if is_strict {
            file.check_for_duplicate_headers()?;
        }
        Ok(file)
    }

    /// Reads lines from `reader`, parses them, and keeps them.
    fn read_from<F: BufRead>(&mut self, mut reader: F) -> Result<(), ParseError> {
        let mut loc = ErrorLocation::new(self.filename);
        let mut buffer = String::new();
        loop {
            // Increase the line number first. If we did this at the
            // end of the loop, an error in the first line would be
            // reported as "error in line 0".
            loc.lineno += 1;
            let num_bytes = reader.read_line(&mut buffer).context(loc)?;
            if num_bytes == 0 {
                break;
            }
            let line = buffer.parse::<InputLine>().context(loc)?;
            self.lines.push(line);
            buffer.clear();
        }
        Ok(())
    }

    /// Returns an error if two header lines have the same content.
    fn check_for_duplicate_headers(&self) -> Result<(), ParseError> {
        let mut seen_headers = HashMap::new();
        let mut loc = ErrorLocation::new(self.filename);
        for line in self.lines.iter() {
            loc.lineno += 1;
            // We are only interested in header lines. If a header line
            // has not been seen before, we note its content and line
            // number. If we *have* seen it before, we build an error
            // from the header line's content, the current line number
            // and the line number of the previous occurrence.
            if let Some(header) = line.header() {
                match seen_headers.entry(header) {
                    Entry::Vacant(entry) => {
                        entry.insert(loc.lineno);
                    },
                    Entry::Occupied(entry) => {
                        let header = header.to_owned();
                        let previous_lineno = *entry.get();
                        Err(ErrorKind::ScenarioExists(header, previous_lineno))
                            .context(loc)?;
                    },
                }
            }
        }
        Ok(())
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
    fn new<S, E>(loc: ErrorLocation<S>, kind: E) -> Self
    where
        S: AsRef<str>,
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
impl<S, E> From<Context<ErrorLocation<S>, E>> for ParseError
where
    S: AsRef<str>,
    E: Into<ErrorKind>,
{
    fn from(context: Context<ErrorLocation<S>, E>) -> Self {
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
        ScenarioExists(name: String, previous_lineno: usize) {
            description("scenario already exists")
            display(err) -> ("{}: \"{}\" (first occurrence on line {})",
                             err.description(), name, previous_lineno)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;
    use std::io::Cursor;


    fn get_scenarios(contents: &str) -> Result<ScenarioFile, ParseError> {
        ScenarioFile::new(Cursor::new(contents), "<memory>", true)
    }

    fn get_scenarios_lax(contents: &str) -> Result<ScenarioFile, ParseError> {
        ScenarioFile::new(Cursor::new(contents), "<memory>", false)
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
        let file = get_scenarios(file).unwrap();
        let scenarios = file.iter().collect::<Result<Vec<_>, _>>().unwrap();
        let mut scenarios = scenarios.iter();

        let the_scenario = scenarios.next().unwrap();
        let the_variables = [("aaaa", "1"), ("bbbb", "8"), ("cdcd", "complicated value")];
        assert_eq!(the_scenario.name(), "First Scenario");
        assert_vars(&the_scenario, &the_variables);

        let the_scenario = scenarios.next().unwrap();
        let the_variables = [("aaaa", "8"), ("bbbb", "1"), ("cdcd", "lesscomplicated")];
        assert_eq!(the_scenario.name(), "Second Scenario");
        assert_vars(&the_scenario, &the_variables);

        let the_scenario = scenarios.next().unwrap();
        assert_eq!(the_scenario.name(), "Third Scenario");
        assert_vars(&the_scenario, &[]);

        assert!(scenarios.next().is_none());
    }

    #[test]
    fn test_non_unique_names() {
        let expected_message = "<memory>:5: scenario already exists: \"second\" (first occurrence \
                                on line 2)";
        let file = "[first]\n[second]\n\n[third]\n[second]";
        assert_eq!(
            get_scenarios(file).unwrap_err().to_string(),
            expected_message
        );
    }

    #[test]
    fn test_non_unique_names_allowed() {
        let file = get_scenarios_lax("[first]\n[second]\n\n[third]\n[second]").unwrap();
        let scenarios = file.iter().collect::<Result<Vec<_>, _>>().unwrap();
        let names: Vec<&str> = scenarios.iter().map(Scenario::name).collect();
        assert_eq!(names, ["first", "second", "third", "second"]);
    }

    #[test]
    fn test_invalid_variable_def() {
        let expected_message = "<memory>:2: syntax error: no equals sign \"=\" in variable \
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
        let expected_message = "<memory>:2: syntax error: closing bracket \"]\" does not end the \
                                line: \"[key] = value\"";
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
