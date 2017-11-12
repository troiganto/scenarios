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
        let result = self.command
            .spawn()
            .map_err(SpawnErrorTag)
            .context((&name, &program))
            .map_err(Error::from);
        match result {
            Ok(child) => Ok(RunningChild { name, program, child, token }),
            Err(err) => Err((err, token)),
        }
    }

    /// Like `spawn`, but returns the `PoolToken` in case of errors.
    ///
    /// If this function fails, it returns `token` to the given `stock`
    /// instead of returning it by-value. This allows this function to
    /// return a proper `Result` whose `Err` implements `Error`.
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
    program: String,
    child: Child,
    token: PoolToken,
}

impl RunningChild {
    /// Waits for this child to finish running.
    ///
    /// # Errors
    /// Waiting can theoretically fail.
    pub fn wait(&mut self) -> Result<()> {
        self.child
            .wait()
            .map_err(WaitErrorTag)
            .context((&self.name, &self.program))?;
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
        let status = self.child
            .try_wait()
            .map_err(WaitErrorTag)
            .context((&self.name, &self.program))?;
        Ok(status.is_some())
    }

    /// Turns the `RunningChild` into a `FinishedChild`.
    ///
    /// This also returns the `PoolToken` that the child had.
    ///
    /// # Errors
    /// Waiting can theoretically fail. The `PoolToken` is returned in
    /// any case.
    pub fn finish(mut self) -> (Result<FinishedChild>, PoolToken) {
        let result = self.child
            .wait()
            .map_err(WaitErrorTag)
            .context((&self.name, &self.program))
            .map_err(Error::from);
        let Self { name, program, token, .. } = self;
        let result = result.map(|status| FinishedChild { name, program, status });
        (result, token)
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
    /// signal, this returns an error of kind `ChildFailed`.
    pub fn into_result(self) -> Result<()> {
        if self.status.success() {
            Ok(())
        } else {
            Err(self.status)
                .context((&self.name, &self.program))
                .map_err(Error::from)
        }
    }
}


/// Wrapper type that tags an `io::Error` as coming from a call to
/// `spawn()`.
///
/// This disambiguates the conversion from `io::Error` to `ErrorKind`.
#[derive(Debug)]
struct SpawnErrorTag(io::Error);


/// Wrapper type that tags an `io::Error` as coming from a call to
/// `wait()`.
///
/// This disambiguates the conversion from `io::Error` to `ErrorKind`.
#[derive(Debug)]
struct WaitErrorTag(io::Error);


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
        Some(&self.kind)
    }
}

/// Converts from `Context` to `Error``.
///
/// Because `name` is a field of `Error`, but `program` is a field of
/// `ErrorKind`, it is difficult to pass both via `Context`. This
/// implementation abstracts that difficult fact away by accepting a
/// tuple as a context.
impl<Name, Program, RawError> From<Context<(Name, Program), RawError>> for Error
where
    Name: AsRef<str>,
    Program: AsRef<str>,
    ErrorKind: From<Context<Program, RawError>>
{
    fn from(context: Context<(Name, Program), RawError>) -> Self {
        let (name, program) = context.0;
        let raw_error = context.1;
        let kind = ErrorKind::from(Context(program, raw_error));
        Error { name: name.as_ref().to_owned(), kind }
    }
}


quick_error! {
    /// The kinds of errors that can be caused in this module.
    #[derive(Debug)]
    pub enum ErrorKind {
        SpawnError(program: String, err: io::Error) {
            description("could not execute command")
            display(self_) -> ("{}: {}", self_.description(), program)
            cause(err)
            context(program: AsRef<str>, err: SpawnErrorTag)
                -> (program.as_ref().to_owned(), err.0)
        }
        WaitError(program: String, err: io::Error) {
            description("error while waiting for job to finish")
            display(self_) -> ("{}: {}", self_.description(), program)
            cause(err)
            context(program: AsRef<str>, err: WaitErrorTag) -> (program.as_ref().to_owned(), err.0)
        }
        ChildFailed(program: String, status: ExitStatus) {
            description("command returned non-zero exit status")
            display("command \"{}\" exited with {}", program, status)
            context(program: AsRef<str>, status: ExitStatus)
                -> (program.as_ref().to_owned(), status)
        }
    }
}
