
use std::error::Error;
use std::fmt::{self, Display};
use std::process::ExitStatus;

/// Extension trait to patch types with an `into_result` nethod.
///
/// This type is used to patch `ExitStatus`.
pub trait IntoResult {
    /// The corresponding success type.
    type Success;

    /// The corresponding error type.
    type Error;

    /// Converts `self` into a given result type.
    fn into_result(self) -> Result<Self::Success, Self::Error>;
}

impl IntoResult for ExitStatus {
    type Success = ();
    type Error = CommandFailed;

    /// Converts an `ExitStatus` into a `Result`.
    ///
    /// If the status indicates success, `Ok(())`` is returned.
    /// Otherwise, `Err(Error::CommandFailed)` is returned.
    fn into_result(self) -> Result<Self::Success, Self::Error> {
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
