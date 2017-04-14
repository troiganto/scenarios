
use std::error::Error;
use std::fmt::{self, Display};


/// Errors caused during parsing of input lines.
#[derive(Debug)]
pub struct ParseError {
    line: String,
    lineno: Option<u32>,
    filename: Option<String>,
}

impl ParseError {
    pub fn new<S: Into<String>>(line: S) -> Self {
        ParseError {
            line: line.into(),
            lineno: None,
            filename: None,
        }
    }

    pub fn set_lineno(&mut self, lineno: u32) {
        self.lineno = Some(lineno);
    }

    pub fn set_filename<S: Into<String>>(&mut self, filename: S) {
        self.filename = Some(filename.into());
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match (&self.lineno, &self.filename) {
            (&Some(lineno), &Some(ref fname)) => {
                write!(f, "in file {}, line {}: {}", fname, lineno, self.line)
            }
            (&None, &Some(ref fname)) => write!(f, "in file {}: {}", fname, self.line),
            (&Some(lineno), &None) => write!(f, "in line {}: {}", lineno, self.line),
            (&None, &None) => write!(f, "{}", self.line),
        }
    }
}


impl Error for ParseError {
    fn description(&self) -> &str {
        "parse error while reading scenarios file"
    }
    fn cause(&self) -> Option<&Error> {
        None
    }
}


/// Errors caused during building a scenario.
#[derive(Debug)]
pub enum ScenarioError {
    InvalidName(String),
    InvalidVariable(String),
    DuplicateVariable(String),
}


impl Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ScenarioError::*;

        match *self {
            InvalidName(ref name) => write!(f, "{}: {:?}", self.description(), name),
            InvalidVariable(ref name) => write!(f, "{}: {:?}", self.description(), name),
            DuplicateVariable(ref name) => write!(f, "{}: {:?}", self.description(), name),
        }
    }
}


impl Error for ScenarioError {
    fn description(&self) -> &str {
        use self::ScenarioError::*;

        match *self {
            InvalidName(_) => "invalid name",
            InvalidVariable(_) => "invalid variable name",
            DuplicateVariable(_) => "duplicate variable",
        }
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}
