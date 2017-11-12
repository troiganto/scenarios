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


use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::io;
use std::process::{Command, Child, ExitStatus};

use quick_error::{Context, ResultExt};

use super::tokens::{PoolToken, TokenStock};


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
pub struct PreparedChild {
    /// The name of the corresponding scenario.
    name: String,
    /// The name of the running scenario.
    program: String,
    command: Command,
}

impl PreparedChild {
    /// Creates a new `PreparedChild`.
    pub fn new(name: String, program: String, command: Command) -> Self {
        PreparedChild { name, program, command }
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
            Ok(child) => Ok(RunningChild { name, program, child, token }),
            Err(error) => {
                let kind = SpawnError { program, error }.into();
                Err((Error { name, kind }, token))
            },
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


/// Error type of `PreparedChild::spawn()`.
#[derive(Debug)]
pub struct SpawnError {
    program: String,
    error: io::Error,
}

impl StdError for SpawnError {
    fn description(&self) -> &str {
        "error while waiting for job to finish"
    }
    fn cause(&self) -> Option<&StdError> {
        Some(&self.error)
    }
}

impl Display for SpawnError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.description(), self.program)
    }
}

impl<S: Into<String>> From<Context<S, io::Error>> for SpawnError {
    fn from(context: Context<S, io::Error>) -> Self {
        SpawnError {
            program: context.0.into(),
            error: context.1,
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
    program: String,
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
        self._wait().context(self.name.as_str())?;
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
        let finished = self._try_wait().context(self.name.as_str())?;
        Ok(finished)
    }

    /// Turns the `RunningChild` into a `FinishedChild`.
    ///
    /// This also returns the `PoolToken` that the child had.
    ///
    /// # Errors
    /// Waiting can theoretically fail. The `PoolToken` is returned in
    /// any case.
    pub fn finish(mut self) -> (Result<FinishedChild>, PoolToken) {
        let result = self._wait()
            .context(self.name.as_str())
            .map_err(Error::from);
        let Self { name, program, token, .. } = self;
        let result = result.map(|status| FinishedChild { name, program, status });
        (result, token)
    }

    fn _wait(&mut self) -> ::std::result::Result<ExitStatus, WaitError> {
        let status = self.child.wait().context(self.name.as_str())?;
        Ok(status)
    }

    fn _try_wait(&mut self) -> ::std::result::Result<bool, WaitError> {
        let status = self.child.try_wait().context(self.name.as_str())?;
        Ok(status.is_some())
    }
}


/// Error type of `RunningChild::wait()`.
#[derive(Debug)]
pub struct WaitError {
    program: String,
    error: io::Error,
}

impl StdError for WaitError {
    fn description(&self) -> &str {
        "could not execute command"
    }
    fn cause(&self) -> Option<&StdError> {
        Some(&self.error)
    }
}

impl Display for WaitError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: {}", self.description(), self.program)
    }
}

impl<S: Into<String>> From<Context<S, io::Error>> for WaitError {
    fn from(context: Context<S, io::Error>) -> Self {
        WaitError {
            program: context.0.into(),
            error: context.1,
        }
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
    program: String,
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
            let kind = ChildFailed {
                    program: self.program,
                    status: self.status,
                }
                .into();
            Err(Error { name: self.name, kind })
        }
    }
}


/// Error type of `FinishedChild::into_result()`.
#[derive(Debug)]
pub struct ChildFailed {
    program: String,
    status: ExitStatus,
}

impl StdError for ChildFailed {
    fn description(&self) -> &str {
        "command returned non-zero exit status"
    }
    fn cause(&self) -> Option<&StdError> {
        None
    }
}

impl Display for ChildFailed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "command \"{}\" exited with {}",
            self.program,
            self.status
        )
    }
}

impl<S: Into<String>> From<Context<S, ExitStatus>> for ChildFailed {
    fn from(context: Context<S, ExitStatus>) -> Self {
        ChildFailed {
            program: context.0.into(),
            status: context.1,
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

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: \"{}\"", self.description(), self.name)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        "scenario did not finish successfully"
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
        SpawnError(err: SpawnError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        WaitError(err: WaitError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        ChildFailed(err: ChildFailed) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
    }
}
