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


use std::{
    collections::hash_map::{Entry, HashMap},
    ffi::OsStr,
    fs::File,
    io::{self, BufRead},
    path::Path,
};

use failure::{Error, Fail, ResultExt};

use super::{inputline::InputLine, location::ErrorLocation, scenario::Scenario};


/// Type that represents a scenario file.
///
/// Creating an instance of this type means to open a file or other
/// `Read`able object and read a sequence of input lines from it. When
/// producing [`Scenario`]s from this file, these input lines are
/// parsed and turned into [`Scenario`]s.
///
/// [`Scenario`]s borrow from this type. Its prime purpose is to serve
/// as the owner of all the strings [`Scenario`] uses. This separation
/// allows us to avoid a lot of `String` copies, operating on `str`
/// slices instead.
///
/// [`Scenario`]: ./struct.Scenario.html
#[derive(Debug)]
pub struct ScenarioFile<'a> {
    filename: &'a Path,
    lines: Vec<InputLine>,
}

impl<'a> ScenarioFile<'a> {
    /// Takes a command-line argument and reads a file from it.
    ///
    /// If `path` equals `"-"`, this reads scenarios from standard
    /// input. Otherwise, it reads from the regular file located at
    /// `path`.
    ///
    /// If `is_strict` is `true`, this function checks after reading
    /// whether any two scenarios in it have the same name. If they do,
    /// this function returns an error. If `is_strict` is `false`, the
    /// check is not performed.
    ///
    /// Note that this call reads all lines in the file into memory,
    /// but does not create any [`Scenario`]s yet. This only happens
    /// when iterating over the file.
    ///
    /// # Errors
    /// This function may fail for any of the following reasons:
    ///
    /// 1. The file located at `path` cannot be opened.
    /// 2. Reading from the file fails at any point.
    /// 3. The file breaks the syntax of scenario files.
    /// 4. The file defines two scenarios with the same name. (only if
    /// `is_strict` is `true`).
    ///
    /// [`Scenario`]: ./struct.Scenario.html
    pub fn from_cl_arg(path: &OsStr, is_strict: bool) -> Result<ScenarioFile, Error> {
        let stdin = io::stdin();
        if path == Path::new("-") {
            Self::new(stdin.lock(), "<stdin>".as_ref(), is_strict)
        } else {
            let file = File::open(path).with_context(|_| ErrorLocation::new(path.to_owned()))?;
            let file = io::BufReader::new(file);
            Self::new(file, path.as_ref(), is_strict)
        }
    }

    /// Reads scenarios from a given buffered reader.
    fn new<F>(reader: F, filename: &Path, is_strict: bool) -> Result<ScenarioFile, Error>
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
    fn read_from<F: BufRead>(&mut self, mut reader: F) -> Result<(), Error> {
        let mut loc = ErrorLocation::new(self.filename);
        let mut buffer = String::new();
        loop {
            // Increase the line number first. If we did this at the
            // end of the loop, an error in the first line would be
            // reported as "error in line 0".
            loc.lineno += 1;
            let num_bytes = reader
                .read_line(&mut buffer)
                .with_context(|_| loc.to_owned())?;
            if num_bytes == 0 {
                break;
            }
            let line = buffer
                .parse::<InputLine>()
                .with_context(|_| loc.to_owned())?;
            self.lines.push(line);
            buffer.clear();
        }
        Ok(())
    }

    /// Returns an error if two header lines have the same content.
    fn check_for_duplicate_headers(&self) -> Result<(), Error> {
        let mut seen_headers = HashMap::new();
        let mut loc = ErrorLocation::new(self.filename);
        for line in &self.lines {
            loc.lineno += 1;
            // We are only interested in header lines. If a header line
            // has not been seen before, we note its content and line
            // number. If we *have* seen it before, we build an error
            // from the header line's content, the current line number
            // and the line number of the previous occurrence.
            if let Some(header) = line.as_header() {
                match seen_headers.entry(header) {
                    Entry::Vacant(entry) => {
                        entry.insert(loc.lineno);
                    },
                    Entry::Occupied(prev_lineno_entry) => {
                        let prev_loc = ErrorLocation::with_lineno(
                            self.filename.to_owned(),
                            *prev_lineno_entry.get(),
                        );
                        let err = DuplicateScenarioName(header.to_owned())
                            .context(loc.to_owned())
                            .context(prev_loc)
                            .into();
                        return Err(err);
                    },
                }
            }
        }
        Ok(())
    }

    /// Returns the name of the file that was read.
    ///
    /// For standard input, this is `"<stdin>"`. For any regular file,
    /// this is the path to it.
    pub fn filename(&self) -> &Path {
        self.filename
    }

    /// Returns an iterator that creates [`Scenario`]s from the file.
    ///
    /// [`Scenario`]: ./struct.Scenario.html
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


/// An iterator that reads [`Scenario`]s from a [`ScenarioFile`].
///
/// [`Scenario`]: ./struct.Scenario.html
/// [`ScenarioFile`]: ./struct.ScenarioFile.html
#[derive(Debug, Clone)]
pub struct ScenariosIter<'a> {
    location: ErrorLocation<&'a Path>,
    lines: &'a [InputLine],
}

impl<'a> ScenariosIter<'a> {
    /// Creates a new instance.
    fn new(filename: &'a Path, lines: &'a [InputLine]) -> Self {
        let location = ErrorLocation::new(filename);
        ScenariosIter { location, lines }
    }

    /// Continue parsing the file until the next header line or EOF.
    ///
    /// This function returns the scenario belonging to the current
    /// header line. It is private and merely a convenience helper for
    /// [`next()`].
    ///
    /// # Errors
    /// This may fail either with a [`ScenarioError`] or an
    /// [`UnexpectedVarDef`].
    ///
    /// [`next()`]: #method.next
    /// [`ScenarioError`]: ./enum.ScenarioError.html
    /// [`UnexpectedVarDef`]: ./struct.UnexpectedVarDef.html
    fn next_scenario(&mut self) -> Result<Option<Scenario<'a>>, Error> {
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
    /// incremented, but a [`UnexpectedVarDef`] is returned.
    ///
    /// [`UnexpectedVarDef`]: ./struct.UnexpectedVarDef.html
    fn next_header_line(&mut self) -> Result<Option<&'a str>, UnexpectedVarDef> {
        while let Some(line) = self.lines.get(self.location.lineno) {
            self.location.lineno += 1;
            if let Some(header) = line.as_header() {
                return Ok(Some(header));
            } else if let Some((name, _)) = line.as_definition() {
                return Err(UnexpectedVarDef(name.to_owned()));
            }
        }
        Ok(None)
    }

    /// Fetches the next definition line.
    ///
    /// Comment lines are skipped over. This returns `None` if the
    /// end-of-file is reached or a header line is found. (The header
    /// line is *not* extracted!) Otherwise, the split variable
    /// definition is returned.
    fn next_definition_line(&mut self) -> Option<(&'a str, &'a str)> {
        while let Some(line) = self.lines.get(self.location.lineno) {
            if line.is_header() {
                // Leave *without* moving to the next line.
                break;
            } else {
                self.location.lineno += 1;
                if let Some(parts) = line.as_definition() {
                    return Some(parts);
                }
            }
        }
        None
    }
}

impl<'a> Iterator for ScenariosIter<'a> {
    type Item = Result<Scenario<'a>, Error>;

    /// Reads the next scenario from the scenario file.
    ///
    /// # Errors
    ///
    /// This may fail if the scenario's definition is bad:
    ///
    /// - The scenario cannot be build, or
    /// - a variable was defined outside of any scenario.
    fn next(&mut self) -> Option<Self::Item> {
        match self
            .next_scenario()
            .with_context(|_| self.location.to_owned())
        {
            Ok(None) => None,
            Ok(Some(scenario)) => Some(Ok(scenario)),
            Err(context) => Some(Err(Error::from(context))),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<'a> ExactSizeIterator for ScenariosIter<'a> {
    fn len(&self) -> usize {
        self.lines
            .iter()
            .skip(self.location.lineno)
            .filter(|line| line.is_header())
            .count()
    }
}


/// The error returned for unexpected variable definitions.
///
/// A variable definition is unexpected if it appears in the scenario
/// file before any scenario has been declared – i.e. before the first
/// header line.
#[derive(Debug, Fail)]
#[fail(display = "variable definition before the first header: \"{}\"", _0)]
pub struct UnexpectedVarDef(String);


/// The error returned if two scenarios share the same name.
#[derive(Debug, Fail)]
#[fail(display = "duplicate scenario name: \"{}\"", _0)]
pub struct DuplicateScenarioName(String);


#[cfg(test)]
mod tests {
    use super::*;

    use std::{collections::HashSet, io::Cursor};


    fn get_scenarios(contents: &str) -> Result<ScenarioFile, Error> {
        ScenarioFile::new(Cursor::new(contents), Path::new("<memory>"), true)
    }

    fn get_scenarios_lax(contents: &str) -> Result<ScenarioFile, Error> {
        ScenarioFile::new(Cursor::new(contents), Path::new("<memory>"), false)
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
        let err = get_scenarios("[first]\n[second]\n\n[third]\n[second]").unwrap_err();
        let mut err = err.cause();
        assert_eq!(err.to_string(), "in <memory>:2");
        err = err.cause().unwrap();
        assert_eq!(err.to_string(), "in <memory>:5");
        err = err.cause().unwrap();
        assert_eq!(err.to_string(), "duplicate scenario name: \"second\"");
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
        let err = get_scenarios("[scenario]\nthe bad line").unwrap_err();
        let mut err = err.cause();
        assert_eq!(err.to_string(), "in <memory>:2");
        err = err.cause().unwrap();
        assert_eq!(
            err.to_string(),
            "no equals sign \"=\" in variable definition: \"the bad line\""
        );
    }

    #[test]
    fn test_variable_already_defined() {
        let file = get_scenarios("[scenario]\na = b\na = c\n").unwrap();
        let err = file.iter().collect::<Result<Vec<_>, _>>().unwrap_err();
        let mut err = err.cause();
        assert_eq!(err.to_string(), "in <memory>:3");
        err = err.cause().unwrap();
        assert_eq!(err.to_string(), "variable already defined: \"a\"");
    }

    #[test]
    fn test_invalid_header() {
        let err = get_scenarios("[scenario]\n[key] = value").unwrap_err();
        let mut err = err.cause();
        assert_eq!(err.to_string(), "in <memory>:2");
        err = err.cause().unwrap();
        assert_eq!(
            err.to_string(),
            "closing bracket \"]\" does not end the line: \"[key] = value\""
        );
    }

    #[test]
    fn test_invalid_variable_name() {
        let file = get_scenarios("[scenario]\nß = ss").unwrap();
        let err = file.iter().collect::<Result<Vec<_>, _>>().unwrap_err();
        let mut err = err.cause();
        assert_eq!(err.to_string(), "in <memory>:2");
        err = err.cause().unwrap();
        assert_eq!(err.to_string(), "invalid variable name: \"ß\"");
    }

    #[test]
    fn test_invalid_scenario_name() {
        let file = get_scenarios("[scenario]\na = b\n[]\n").unwrap();
        let err = file.iter().collect::<Result<Vec<_>, _>>().unwrap_err();
        let mut err = err.cause();
        assert_eq!(err.to_string(), "in <memory>:3");
        err = err.cause().unwrap();
        assert_eq!(err.to_string(), "invalid scenario name: \"\"");
    }

    #[test]
    fn test_unexpected_vardef() {
        let file = r"
        # second line
        # third line

        # fifth line
        a = b
        ";
        let file = get_scenarios(file).unwrap();
        let err = file.iter().collect::<Result<Vec<_>, _>>().unwrap_err();
        let mut err = err.cause();
        assert_eq!(err.to_string(), "in <memory>:6");
        err = err.cause().unwrap();
        assert_eq!(
            err.to_string(),
            "variable definition before the first header: \"a\""
        );
    }


    #[test]
    fn test_exact_size_iterator() {
        let file = get_scenarios("[first]\n[second]\n\n[third]\n[fourth]").unwrap();
        let mut scenarios = file.iter();
        assert_eq!(scenarios.len(), 4);
        assert_eq!(scenarios.size_hint(), (4, Some(4)));
        scenarios.next();
        assert_eq!(scenarios.len(), 3);
    }

}
