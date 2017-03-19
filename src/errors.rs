
use std::io;
use std::fmt::{self, Display};
use std::error::Error;

#[derive(Debug)]
pub enum ParseError {

    DuplicateName(String),
    DuplicateVariable(String),

    InvalidName(String),
    InvalidVariable(String),
    SyntaxError(String),

    NoScenario,
    UnexpectedVarDef(String),
    IoError(io::Error),
}


impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ParseError::*;

        match *self {
            DuplicateName(ref name) => write!(f, "{}: {:?}", self.description(), name),
            DuplicateVariable(ref name) => write!(f, "{}: {:?}", self.description(), name),
            InvalidName(ref name) => write!(f, "{}: {:?}", self.description(), name),
            InvalidVariable(ref name) => write!(f, "{}: {:?}", self.description(), name),
            SyntaxError(ref line) => write!(f, "{}: {:?}", self.description(), line),
            NoScenario => write!(f, "{}", self.description()),
            UnexpectedVarDef(ref vardef) => write!(f, "{}: {:?}", self.description(), vardef),
            IoError(ref err) => err.fmt(f),
        }
    }
}


impl Error for ParseError {
    fn description(&self) -> &str {
        use self::ParseError::*;

        match *self {
            DuplicateName(_) => "duplicate name",
            InvalidName(_) => "invalid name",
            DuplicateVariable(_) => "duplicate variable",
            InvalidVariable(_) => "invalid variable name",
            SyntaxError(_) => "no '=' sign in variable definition",
            NoScenario => "no scenario found in file",
            UnexpectedVarDef(_) => "variable definition outside of scenario",
            IoError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        use self::ParseError::*;

        match *self {
            IoError(ref err) => err.cause(),
            _ => None,
        }
    }
}


impl From<io::Error> for ParseError {
    fn from(err: io::Error) -> Self { ParseError::IoError(err) }
}
