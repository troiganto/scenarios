
use std::io::{self, BufRead};
use std::collections::hash_map::{self, HashMap};

use inputline::InputLine;
use errors::ParseError;


fn is_alnum_identifier(s: &str) -> bool {
    use regex::Regex;
    lazy_static!{
        static ref RE: Regex = Regex::new("^[_[:alpha:]][[:word:]]*$").unwrap();
    }
    RE.is_match(s)
}


/// Named set of environment variable definitions.
///
/// A scenario has a name and a set of environment variable definitions.
/// Each definition has an associated variable name and the corresponding
/// value, both strings. A variable name must follow the rules for regular
/// C identifiers.
///
/// `Scenario`s are created through the `iter_from_file()` function.
#[derive(Clone, Debug)]
pub struct Scenario {
    name: String,
    variables: HashMap<String, String>,
}

impl Scenario {
    /// Iterate over all scenarios described in an input file.
    ///
    /// This reads the input file `file` and lazily parses it as a list of
    /// scenario descriptions. The scenarios are yielded by the iterator
    /// returned by this function.
    ///
    /// # Errors
    /// This call fails if the iterator cannot be constructed. This is the
    /// case if the passed file does not contain any scenarios, if there
    /// is a syntax error before finding the first scenario or if any I/O
    /// error occurs.
    pub fn iter_from_file<F: BufRead>(file: F) -> Result<Iter<F>, ParseError> { Iter::new(file) }

    /// Returns an iterator over all variable names.
    pub fn variable_names(&self) -> hash_map::Keys<String, String> {
        self.variables.keys()
    }

    /// Returns an iterator over all variables.
    pub fn variable(&self) -> hash_map::Iter<String, String> {
        self.variables.iter()
    }

    /// Returns an iterator over all variables.
    pub fn into_variable(self) -> hash_map::IntoIter<String, String> {
        self.variables.into_iter()
    }

    /// Returns `true` if the variable already exists in this scenario.
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Returns the value of variable named `name`, if it exists.
    pub fn get_variable(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(String::as_str)
    }

    /// Returns the name of the scenario.
    pub fn name(&self) -> &str { &self.name }

    /// Merges another scenario into this one.
    ///
    /// This combines the names and variables of both scenarios.
    /// The names get combined with a ", " (comma+space). Variables are
    /// combined by adding the `other` `HashMap` into `self`'s.
    /// If both scenarios define the same variable, the value of
    /// `other`'s takes precedence.
    pub fn merge(&mut self, other: Scenario) {
        // Merge names.
        self.name.reserve(other.name.len()+2);
        self.name.push_str(", ");
        self.name.push_str(&other.name);
        // Merge variables.
        for (key, value) in other.into_variable() {
            self.variables.insert(key, value);
        }
    }

    /// Creates a new scenario.
    ///
    /// This method is private. Use `iter_from_file()` instead.
    ///
    /// # Errors
    /// This call fails with `ParseError::InvalidName` if `name`
    /// is the empty string or contains a comma.
    fn new<S>(name: S) -> Result<Self, ParseError> where S: Into<String> {
        let name = name.into();
        if name.is_empty() || name.contains(',') {
            return Err(ParseError::InvalidName(name));
        }
        Ok(Scenario{name: name, variables: HashMap::new()})
    }

    /// Adds another variable definition of the current set.
    ///
    /// # Errors
    /// This call fails with `ParseError::InvalidVariable` if `name` is
    /// not a valid variable name (`[A-Za-z_][A-Za-z0-9_]+`). It fails
    /// with `ParseError::DuplicateVariable` if a variable of this name
    /// already has been added to the scenario.
    fn add_variable<S1, S2>(&mut self, name: S1, value: S2) -> Result<(), ParseError>
        where S1: Into<String>, S2: Into<String>
    {
        let name = name.into();
        let value = value.into();
        if self.has_variable(&name) {
            Err(ParseError::DuplicateVariable(name))
        } else if !is_alnum_identifier(&name) {
            Err(ParseError::InvalidVariable(name))
        } else {
            self.variables.insert(name, value);
            Ok(())
        }
    }
}


/// The iterator returned by `Scenario::iter_from_file()`.
#[derive(Debug)]
pub struct Iter<F: BufRead> {
    /// The wrapped iterator of input file lines.
    lines: io::Lines<F>,
    /// Intermediate buffer for the next scenario's name.
    next_header: Option<String>,
}


impl<F: BufRead> Iter<F> {
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

impl<F: BufRead> Iterator for Iter<F> {
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
    fn test_is_alnum_identifier() {
        assert!(is_alnum_identifier("_"));
        assert!(is_alnum_identifier("SomeValue"));
        assert!(is_alnum_identifier("ALL_CAPS_AND_9"));
        assert!(is_alnum_identifier("l111"));
        assert!(is_alnum_identifier("__init__"));


        assert!(!is_alnum_identifier(""));
        assert!(!is_alnum_identifier("some value"));
        assert!(!is_alnum_identifier("Mörder"));
        assert!(!is_alnum_identifier("7"));
        assert!(!is_alnum_identifier("1a"));
        assert!(!is_alnum_identifier("🍣"));
    }


    #[test]
    fn test_scenario_new() {
        assert!(Scenario::new("A Name").is_ok());
        assert!(Scenario::new("666").is_ok());

        assert!(Scenario::new("a, b").is_err());
        assert!(Scenario::new("").is_err());
    }


    #[test]
    fn test_scenario_add_variable() {
        let mut s = Scenario::new("name").unwrap();
        // Adding a variable.
        assert!(s.add_variable("key", "value").is_ok());
        // Values may contain spaces.
        assert!(s.add_variable("key2", "a value").is_ok());
        // The same variable must not be added twice.
        assert!(s.add_variable("key", "value").is_err());
        // Variable names must be C identifiers.
        assert!(s.add_variable("a key", "value").is_err());
        // Check that adding occurred.
        assert!(s.has_variable("key"));
        assert!(!s.has_variable("a key"));
    }


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
        let output = Scenario::iter_from_file(file).unwrap();

        let s = output.next().unwrap();
        assert_eq!(s.name(), "First Scenario");
    }
}
