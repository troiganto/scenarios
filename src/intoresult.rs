
use std::error::Error;
use std::fmt::{self, Display};
use std::process::ExitStatus;

/// Extension trait that is used to patch `ExitStatus`.
pub trait IntoResult<T, E> {
    /// Converts `self` into a given result type.
    fn into_result(self) -> Result<T, E>;
}

impl IntoResult<(), CommandFailed> for ExitStatus {
    /// Converts an `ExitStatus` into a `Result`.
    ///
    /// If the status indicates success, `Ok(())`` is returned.
    /// Otherwise, `Err(Error::CommandFailed)` is returned.
    fn into_result(self) -> Result<(), CommandFailed> {
        if self.success() {
            Ok(())
        } else {
            Err(CommandFailed(self))
        }
    }
}


/// Error type that indicates an unsuccessful `ExitStatus`.
#[derive(Debug)]
pub struct CommandFailed(ExitStatus);

impl Display for CommandFailed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "command returned non-zero {}", self.0)
    }
}

impl Error for CommandFailed {
    fn description(&self) -> &str {
        "command returned non-zero exit code"
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}
