
use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::io;
use std::process::{Command, Child, ExitStatus};

use quick_error::{Context, ResultExt};

use super::pool::{PoolToken, TokenStock};


/// Convenience alias for `std::result::Result`.
pub type Result<T> = ::std::result::Result<T, Error>;

/// Wrapper type that combines `std::process::Command` with a name.
///
/// This type is returned by `CommandLine` and represents a process
/// that is ready to start. Starting it requires a `PoolToken`,
/// however, to limit the number of processes that can run in parallel.
///
/// Note that the fields `name` and `program` are only used to provide
/// meaningful error messages if something goes wrong.
#[derive(Debug)]
pub struct PreparedChild<'a> {
    /// The name of the corresponding scenario.
    name: String,
    /// The name of the running scenario.
    program: &'a str,
    command: Command,
}

impl<'a> PreparedChild<'a> {
    /// Creates a new `PreparedChild`.
    pub fn new(name: String, program: &'a str, command: Command) -> Self {
        PreparedChild {
            name,
            program,
            command,
        }
    }

    /// Turns the `PreparedChild` into a `RunningChild`.
    ///
    /// This starts the process wrapped by `self` and combines the
    /// running process with the passed token into a `RunningChild`.
    ///
    /// # Errors
    /// Spawning a process can fail. In such a case, this function
    /// returns both the error that occurred, and the passed
    /// `PoolToken`. This ensures that no token is lost.
    pub fn spawn(
        mut self,
        token: PoolToken,
    ) -> ::std::result::Result<RunningChild, (Error, PoolToken)> {
        let name = self.name;
        let program = self.program;
        match self.command.spawn() {
            Ok(child) => Ok(RunningChild { name, child, token }),
            Err(err) => Err((Error::with_spawn_error(name, program, err), token)),
        }
    }

    /// Like `spawn`, but returns the `PoolToken` in case of errors.
    ///
    /// If this function fails, it returns `token` to the given `stock`
    /// instead of returning it by-value. This allows this function to
    /// return a proper `io::Result`.
    pub fn spawn_or_return_token(
        self,
        token: PoolToken,
        stock: &mut TokenStock,
    ) -> Result<RunningChild> {
        match self.spawn(token) {
            Ok(child) => Ok(child),
            Err((err, token)) => {
                stock.return_token(token);
                Err(err)
            },
        }
    }
}


/// Wrapper type combining `std::process::Child` with name and token.
///
/// This type is returned by `PreparedChild::spawn` and represents a
/// process that is currently running. The correct process is to wait
/// on it and then call `RunningChild::finish()`.
#[derive(Debug)]
pub struct RunningChild {
    name: String,
    child: Child,
    token: PoolToken,
}

impl RunningChild {
    /// Waits for this child to finish running.
    ///
    /// # Errors
    /// Waiting can theoretically fail. In that case, the name of this
    /// child is copied into the error type.
    pub fn wait(&mut self) -> Result<()> {
        self.child.wait().context(self.name.as_str())?;
        Ok(())
    }

    /// Checks whether this child has finished running.
    ///
    /// This waits for the child in a non-blocking manner. If it has
    /// finished running, this returns `Ok(true)`. If the child is
    /// still running, this returns `Ok(false)`.
    ///
    /// # Errors
    /// Waiting can theoretically fail.
    pub fn is_finished(&mut self) -> Result<bool> {
        Ok(
            self.child
                .try_wait()
                .context(self.name.as_str())?
                .is_some(),
        )
    }

    /// Turns the `RunningChild` into a `FinishedChild`.
    ///
    /// This also returns the `PoolToken` that the child had.
    ///
    /// # Errors
    /// Waiting can theoretically fail. The `PoolToken` is returned in
    /// any case.
    pub fn finish(mut self) -> (Result<FinishedChild>, PoolToken) {
        let name = self.name;
        let result = match self.child.wait() {
            Ok(status) => Ok(FinishedChild { name, status }),
            Err(err) => Err(Error::with_wait_error(name, err)),
        };
        (result, self.token)
    }
}


/// Wrapper type combining `std::process::ExitStatus` with a name.
///
/// This type is returned by `RunningChild::finish` and represents a
/// process that has finished running. It can be turned into a `Result`
/// to check whether the child process had exited successfully.
#[derive(Debug)]
pub struct FinishedChild {
    name: String,
    status: ExitStatus,
}

impl FinishedChild {
    /// Checks whether the child process had exited successfully.
    ///
    /// This inspects the wrapped `ExitStatus` and returns `Ok(())` if
    /// the child exited sucessfully.
    ///
    /// # Errors
    /// If the child exited with a non-zero exit status or through a
    /// signal, this returns `Err(Error::ChildFailed(status))`.
    pub fn into_result(self) -> Result<()> {
        if self.status.success() {
            Ok(())
        } else {
            Err(Error::with_exit_status(self.name, self.status))
        }
    }
}


/// The error type used by this module.
///
/// This type essentially ties a regular error (contained in
/// `ErrorKind`) together with the name of the child that caused it.
#[derive(Debug)]
pub struct Error {
    name: String,
    kind: ErrorKind,
}

impl Error {
    /// Create an error of kind `SpawnError`.
    fn with_spawn_error<S1, S2>(name: S1, program: S2, err: io::Error) -> Self
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        Error {
            name: name.into(),
            kind: ErrorKind::SpawnError(program.into(), err),
        }
    }

    /// Create an error of kind `WaitError`.
    fn with_wait_error<S: Into<String>>(name: S, err: io::Error) -> Self {
        Error {
            name: name.into(),
            kind: ErrorKind::WaitError(err),
        }
    }

    /// Create an error of kind `ChildFailed`.
    fn with_exit_status<S: Into<String>>(name: S, status: ExitStatus) -> Self {
        Error {
            name: name.into(),
            kind: ErrorKind::ChildFailed(status),
        }
    }

    /// Accesses the name of the offending child.
    fn name(&self) -> &str {
        &self.name
    }

    /// Accesses the kind of error that occurred.
    fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\n\tin scenario \"{}\"", self.kind, self.name)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        self.kind.description()
    }

    fn cause(&self) -> Option<&StdError> {
        self.kind.cause()
    }
}

/// Allows conversion from `ErrorKind` to `Error` in a `Context`.
///
/// This allows you to enrich an `ErrorKind` with the name of the
/// offending child via `quick_error::ResultExt::context()`.
impl<S: Into<String>, E: Into<ErrorKind>> From<Context<S, E>> for Error {
    fn from(context: Context<S, E>) -> Self {
        let name = context.0.into();
        let kind = context.1.into();
        Error { name, kind }
    }
}


quick_error! {
    /// The kinds of errors that can be caused in this module.
    #[derive(Debug)]
    pub enum ErrorKind {
        SpawnError(program: String, err: io::Error) {
            description("could not execute command")
            display(self_) -> ("{} \"{}\": {}", self_.description(), program, err)
            cause(err)
            context(program: AsRef<str>, err: io::Error) -> (program.as_ref().to_owned(), err)
        }
        WaitError(err: io::Error) {
            description("could not check child process's status")
            display(self_) -> ("{}: {}", self_.description(), err)
            cause(err)
            from()
        }
        ChildFailed(status: ExitStatus) {
            description("command returned non-zero exit status")
            display("command returned non-zero {}", status)
            from()
        }
    }
}
