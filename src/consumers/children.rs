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


use std::{
    ffi::OsStr,
    io, mem,
    process::{Command, ExitStatus},
};

use failure::{Error, ResultExt};
use futures::{Async, Future, Poll};
use tokio_core::reactor::Handle;
use tokio_process::{Child, CommandExt};


/// Wrapper type combining `std::process::Command` with a name.
///
/// This type is returned by [`CommandLine`] and represents a process
/// that is ready to start.
///
/// Note that the names associated with this child are only used to
/// provide meaningful error messages if something goes wrong.
///
/// [`CommandLine`]: ./struct.CommandLine.html
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
        PreparedChild {
            name,
            program,
            command,
        }
    }

    /// Turns `self` into a [`RunningChild`].
    ///
    /// This starts a process from the wrapped `Command`.
    ///
    /// # Errors
    /// This function fails if the wrapped call to
    /// `std::process:Command::spawn()` fails.
    ///
    /// [`RunningChild`]: ./struct.RunningChild.html
    pub fn spawn(mut self, handle: &Handle) -> Result<RunningChild, Error> {
        let name = self.name;
        let program = self.program;
        let child = self
            .command
            .spawn_async(handle)
            .map_err(|cause| {
                let name = program.to_string_lossy().into_owned();
                SpawnFailed { cause, name }
            })
            .with_context(|_| ScenarioNotStarted(name.clone()))?;
        Ok(RunningChild { name, child })
    }
}


/// Wrapper combining an asynchronous [`Child`] with a name.
///
/// This type is returned by [`PreparedChild::spawn()`] and represents
/// a process that is currently running. Because it implements
/// [`Future`], you can wait on it to finish.
///
/// [`Child`]: ../../tokio_process/struct.Child.html
/// [`Future`]: ../../futures/future/trait.Future.html
/// [`PreparedChild::spawn()`]: ./struct.PreparedChild.html#method.spawn
#[derive(Debug)]
pub struct RunningChild {
    name: String,
    child: Child,
}

impl RunningChild {
    fn take_name(&mut self) -> String {
        mem::replace(&mut self.name, String::new())
    }
}

impl Future for RunningChild {
    type Item = FinishedChild;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let status = self
            .child
            .poll()
            .with_context(|_| WaitFailed)
            .with_context(|_| ScenarioFailed(self.take_name()));
        let status = try_ready!(status);
        let name = self.take_name();
        Ok(Async::Ready(FinishedChild { name, status }))
    }
}


/// Wrapper combining an `std::process::ExitStatus` with a name.
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
