
use std::error::Error;
use std::fmt::{self, Display};
use std::io;
use std::process::{Command, Child, ExitStatus};

use super::pool::{PoolToken, TokenStock};


/// Wrapper type that combines `std::process::Command` with a name.
///
/// This type is returned by `CommandLine` and represents a process
/// that is ready to start. Starting it requires a `PoolToken`,
/// however, to limit the number of processes that can run in parallel.
#[derive(Debug)]
pub struct PreparedChild {
    name: String,
    command: Command,
}

impl PreparedChild {
    /// Creates a new `PreparedChild`.
    pub fn new(name: String, command: Command) -> Self {
        PreparedChild { name, command }
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
    pub fn spawn(mut self, token: PoolToken) -> Result<RunningChild, (io::Error, PoolToken)> {
        match self.command.spawn() {
            Ok(child) => {
                let name = self.name;
                Ok(RunningChild { name, child, token })
            },
            Err(err) => Err((err, token)),
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
    ) -> io::Result<RunningChild> {
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
    /// Waiting can theoretically fail with an `io::Error`.
    pub fn wait(&mut self) -> io::Result<()> {
        self.child.wait().map(|_| ())
    }

    /// Checks whether this child has finished running.
    ///
    /// This waits for the child in a non-blocking manner. If it has
    /// finished running, this returns `Ok(true)`. If the child is
    /// still running, this returns `Ok(false)`.
    ///
    /// # Errors
    /// Waiting can theoretically fail with an `io::Error`.
    pub fn is_finished(&mut self) -> io::Result<bool> {
        Ok(self.child.try_wait()?.is_some())
    }

    /// Turns the `RunningChild` into a `FinishedChild`.
    ///
    /// This also returns the `PoolToken` that the child had.
    ///
    /// # Panics
    /// This panics if waiting on the child fails. If you want to avoid
    /// this, `wait` for the child before `finish`ing it.
    pub fn finish(mut self) -> (FinishedChild, PoolToken) {
        let name = self.name;
        let status = self.child
            .wait()
            .expect("waiting on child process failed");
        (FinishedChild { name, status }, self.token)
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
    /// signal, this returns an error.
    pub fn into_result(self) -> Result<(), ChildFailed> {
        if self.status.success() {
            Ok(())
        } else {
            let name = self.name;
            let status = self.status;
            Err(ChildFailed { name, status })
        }
    }
}


/// The error type used by `FinishedChild::into_result()`.
#[derive(Debug)]
pub struct ChildFailed {
    name: String,
    status: ExitStatus,
}

impl Display for ChildFailed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "scenario \"{}\": command returned non-zero {}",
            self.name,
            self.status
        )
    }
}

impl Error for ChildFailed {
    fn description(&self) -> &str {
        "command returned non-zero exit code"
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}
