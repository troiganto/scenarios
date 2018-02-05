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


use std::io;
use std::ffi::OsStr;
use std::process::{Command, Child, ExitStatus};

use failure::{Error, ResultExt};

use super::tokens::{PoolToken, TokenStock};


/// Wrapper type combining `std::process::Command` with a name.
///
/// This type is returned by [`CommandLine`] and represents a process
/// that is ready to start. Starting it requires a [`PoolToken`],
/// however, to limit the number of processes that can run in parallel.
///
/// Note that the names associated with this child are only used to
/// provide meaningful error messages if something goes wrong.
///
/// [`CommandLine`]: ./struct.CommandLine.html
/// [`PoolToken`]: ./struct.PoolToken.html
#[derive(Debug)]
pub struct PreparedChild<'a> {
    name: String,
    program: &'a OsStr,
    command: Command,
}

impl<'a> PreparedChild<'a> {
    /// Creates a new prepared child.
    ///
    /// `name` is the name of the corresponding scenario, `program` is
    /// the name of the program to run. Both names are only used to
    /// build error messages.
    pub fn new(name: String, program: &'a OsStr, command: Command) -> Self {
        PreparedChild { name, program, command }
    }

    /// Turns `self` into a [`RunningChild`].
    ///
    /// This starts a process from the wrapped `Command` and combines
    /// the running process with the passed token into a
    /// [`RunningChild`].
    ///
    /// # Errors
    /// Spawning a process can fail. In such a case, this function
    /// returns both the error that occurred, and the passed
    /// [`PoolToken`]. This ensures that no token is lost.
    ///
    /// [`RunningChild`]: ./struct.RunningChild.html
    /// [`PoolToken`]: ./struct.PoolToken.html
    pub fn spawn(mut self, token: PoolToken) -> Result<RunningChild, (Error, PoolToken)> {
        let name = self.name;
        let program = self.program;
        let result = self.command
            .spawn()
            .map_err(
                |cause| {
                    let name = program.to_string_lossy().into_owned();
                    SpawnFailed { cause, name }
                },
            )
            .with_context(|_| ScenarioNotStarted(name.clone()))
            .map_err(Error::from);
        match result {
            Ok(child) => Ok(RunningChild { name, child, token }),
            Err(err) => Err((err, token)),
        }
    }

    /// Like `spawn`, but may return the token to the [`TokenStock`].
    ///
    /// If this function fails, it returns the given [`PoolToken`] to
    /// the given [`TokenStock`] instead of returning it by-value. This
    /// gives this function a simpler return type.
    ///
    /// [`TokenStock`]: ./struct.TokenStock.html
    /// [`PoolToken`]: ./struct.PoolToken.html
    pub fn spawn_or_return_token(
        self,
        token: PoolToken,
        stock: &mut TokenStock,
    ) -> Result<RunningChild, Error> {
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
/// This type is returned by [`PreparedChild::spawn()`] and represents
/// a process that is currently running.
///
/// [`PreparedChild::spawn()`]: ./struct.PreparedChild.html#method.spawn
#[derive(Debug)]
pub struct RunningChild {
    name: String,
    child: Child,
    token: PoolToken,
}

impl RunningChild {
    /// Checks whether this child has finished running.
    ///
    /// This waits for the child in a non-blocking manner. If it has
    /// finished running, this returns `Ok(true)`. If the child is
    /// still running, this returns `Ok(false)`.
    ///
    /// # Errors
    /// Waiting can theoretically fail. It is not clear under which
    /// circumstances this can happen and what the correct procedure
    /// would be.
    pub fn check_finished(&mut self) -> Result<bool, Error> {
        let status = self.child
            .try_wait()
            .with_context(|_| WaitFailed)
            .with_context(|_| ScenarioFailed(self.name.clone()))?;
        Ok(status.is_some())
    }

    /// Waits for `self` to turn into a [`FinishedChild`].
    ///
    /// This also returns the [`PoolToken`] that the child had.
    ///
    /// # Errors
    /// Waiting can theoretically fail. The [`PoolToken`] is returned
    /// in any case.
    ///
    /// [`FinishedChild`]: ./struct.FinishedChild.html
    /// [`PoolToken`]: ./struct.PoolToken.html
    pub fn finish(mut self) -> (Result<FinishedChild, Error>, PoolToken) {
        let result = self.child
            .wait()
            .with_context(|_| WaitFailed)
            .with_context(|_| ScenarioFailed(self.name.clone()))
            .map_err(Error::from);
        let Self { name, token, .. } = self;
        let result = result.map(|status| FinishedChild { name, status });
        (result, token)
    }
}


/// Wrapper type combining `std::process::ExitStatus` with a name.
///
/// This type is returned by [`RunningChild::finish()`] and represents
/// a process that has finished running. It can be turned into a
/// `Result` to check whether the child process had exited
/// successfully.
///
/// [`RunningChild::finish()`]: ./struct.RunningChild.html#method.finish
#[derive(Debug)]
pub struct FinishedChild {
    name: String,
    status: ExitStatus,
}

impl FinishedChild {
    /// Checks whether the child process had exited successfully.
    ///
    /// This inspects the wrapped `ExitStatus` and returns `Ok(())` if
    /// the child exited sucessfully. Otherwise, an error is returned.
    pub fn into_result(self) -> Result<(), Error> {
        if self.status.success() {
            Ok(())
        } else {
            Err(ChildFailed(self.status))
                .with_context(|_| ScenarioFailed(self.name.clone()))
                .map_err(Error::from)
        }
    }
}


/// The error used to signify that a scenario couldn't even be started.
#[derive(Debug, Fail)]
#[fail(display = "could not start scenario \"{}\"", _0)]
pub struct ScenarioNotStarted(pub String);


/// The error used to say that a scenario was started, but then failed.
#[derive(Debug, Fail)]
#[fail(display = "scenario did not finish successfully: \"{}\"", _0)]
pub struct ScenarioFailed(pub String);


/// Starting up a new child process failed.
#[derive(Debug, Fail)]
#[fail(display = "could not execute command \"{}\"", name)]
pub struct SpawnFailed {
    name: String,
    #[cause]
    cause: io::Error,
}


/// Waiting for a child process's completion failed.
///
/// `std::process::Child::wait()` can fail for any number of
/// platform-dependent reasons. We do the conservative thing and assume
/// the child lost as soon as `wait()` errors even once.
#[derive(Debug, Fail)]
#[fail(display = "failed to wait for job to finish")]
pub struct WaitFailed;


/// A child process has exited in a non-successful manner.
///
/// This can mean a non-zero exit status or exit by signal.
#[derive(Debug, Fail)]
#[fail(display = "job exited with non-zero {}", _0)]
pub struct ChildFailed(ExitStatus);
