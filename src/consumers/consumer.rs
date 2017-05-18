
use std::fmt::{self, Display};
use std::error::Error;

use scenarios::Scenario;

/// A convenience alias.
pub type Result = ::std::result::Result<(), ConsumerError>;


/// Trait for all consumers of scenarios.
///
/// Consumers are objects that actually do something with scenarios.
/// For the most part, this is either printing their name or setting
/// the environment for a command line with them.
pub trait Consumer {
    /// Do something under the given scenario.
    fn consume(&self, scenario: &Scenario) -> Result;
}


/// The common error type of all `Consumer`s.
///
/// Because any implementor of the `Consumer` trait might want to
/// supply their own error type, `ConsumerError` simply boxes the
/// actual error up and derefs to the wrapped `Error` trait object.
#[derive(Debug)]
pub struct ConsumerError(Box<Error>);

impl ConsumerError {
    pub fn new<E: 'static + Error>(err: E) -> Self {
        ConsumerError(Box::new(err))
    }

    pub fn from_box(err: Box<Error>) -> Self {
        ConsumerError(err)
    }

    pub fn as_inner(&self) -> &Error {
        &*self.0
    }

    pub fn into_inner(self) -> Box<Error> {
        self.0
    }
}

impl Display for ConsumerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for ConsumerError {
    fn description(&self) -> &str {
        self.0.description()
    }

    fn cause(&self) -> Option<&Error> {
        Some(self.as_inner())
    }
}

impl From<::std::io::Error> for ConsumerError {
    fn from(err: ::std::io::Error) -> Self {
        ConsumerError::new(err)
    }
}
