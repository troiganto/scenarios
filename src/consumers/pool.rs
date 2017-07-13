
use std::io;
use std::error::Error as StdError;
use std::process::{Child, Command, ExitStatus};
use std::fmt::{self, Display};
use std::collections::VecDeque;

use num_cpus;

use intoresult::{CommandFailed, IntoResult};

/// Type that specifies how many jobs should run in parallel.
///
/// This type is basically a wrapper around `usize`. Only the value
/// `0` is treated specially and gets replaced with the detected
/// numbers of CPU cores on the current machine.
pub struct JobCount(usize);

impl JobCount {
    /// Returns the wrapped value.
    pub fn get(&self) -> usize {
        self.0
    }
}

impl From<usize> for JobCount {
    /// Converts from `int`, replacing `0` with the number of cores.
    fn from(n: usize) -> Self {
        JobCount(if n > 0 { n } else { num_cpus::get() })
    }
}


/// A pool of processes which can run concurrently.
///
/// The pool is used by continously `add`ing `std::process::Command`
/// objects that are used to spawn new processes. The `add()` call will
/// block if the maximum number of concurrent processes has been
/// reached. Once all processes have been submitted, you should call
/// `join()` to wait until they have all finished.
///
/// # Panics
/// Dropping a `Pool` with some processes still queued causes a panic.
/// To avoid this, always call `join` before dropping your `Pool`.
pub struct Pool {
    /// The maximum number of concurrent processes.
    pub num_jobs: JobCount,
    /// The internal queue of added processes.
    queue: VecDeque<Child>,
}

impl Pool {
    /// Creates a pool with `num_jobs` concurrent processes at max.
    ///
    /// If `0` is passed, the automatically determined number of CPU
    /// cores on this machine is used. To disable concurrency, pass
    /// `1`.
    pub fn new<N: Into<JobCount>>(num_jobs: N) -> Self {
        Pool {
            num_jobs: num_jobs.into(),
            queue: VecDeque::new(),
        }
    }

    /// Adds a new process to the pool.
    ///
    /// If the pool is full, this call blocks until a child process
    /// is finished. Then, the a new child process executing `command`
    /// is spawned and added to the pool.
    ///
    /// # Errors
    /// If the pool is not full, this call does not block and always
    /// succeeds.
    ///
    /// If a waited-on process fails, this call fails with
    /// `Error::CommandFailed`. A waited-on process fails e.g. by
    /// returning a non-zero exit status or by aborting through a
    /// signal. The passed `command` gets spawned anyway.
    ///
    /// If an `std::io::Error` occurs, it is retuned. The passed
    /// `Command` is *not* added in this case.
    pub fn add(&mut self, mut command: Command) -> Result<(), Error> {
        let mut result = Ok(());
        // If the queue is full, block until one job is finished.
        if self.queue.len() == self.num_jobs.get() {
            // Abort the function if `find_finished()` fails.
            // But keep the `result` if the child process failed.
            result = self.remove_finished()?
                .expect("queue empty")
                .into_result();
        }
        self.queue.push_back(command.spawn()?);
        result.map_err(From::from)
    }

    /// Waits for all processes left in the queue to finish.
    ///
    /// # Errors
    /// This call fails if any IO error occurs while waiting.
    ///
    /// If a waited-on process fails, this call fails with
    /// `Error::CommandFailed`. A waited-on process fails e.g. by
    /// returning a non-zero exit status or by aborting through a
    /// signal.
    ///
    /// Even if the call fails, all processes are waited on.
    pub fn join(&mut self) -> Result<(), Error> {
        let mut result = Ok(());
        for mut job in self.queue.drain(..) {
            let this_result = job.wait();
            result = result.and_then(|_| this_result?.into_result().map_err(Error::from));
        }
        result
    }

    /// Finds the first finished child process in the queue.
    ///
    /// This sequentially tries to wait for all queued processes. If
    /// a process turns out to have finished, it is immediately removed
    /// from the queue and its `ExitStatus` is returned.
    ///
    /// If all processes are still running, `Ok(None)` is returned.
    ///
    /// # Errors
    /// This call fails if waiting on any process fails. The failing
    /// process may or may not be left in the queue.
    pub fn remove_finished(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.queue.is_empty() {
            return Ok(None);
        }
        let index;
        loop {
            if let Some(i) = self.position_of_finished()? {
                index = i
            }
        }
        self.queue
            .remove(index)
            .expect("index returned by `find_finished` invalid")
            .wait()
            .map(Option::from)
    }

    /// Finds the first finished child process in the queue.
    ///
    /// This sequentially tries to wait for all queued processes. If
    /// a process turns out to have finished, its index in the queue is
    /// returned immediately.
    ///
    /// If all processes are still running, `Ok(None)` is returned.
    ///
    /// # Errors
    /// This call fails if any call to `try_wait()` fails.
    fn position_of_finished(&mut self) -> io::Result<Option<usize>> {
        for (i, job) in self.queue.iter_mut().enumerate() {
            // Return on error or if `job` is finished.
            if job.try_wait()?.is_some() {
                return Ok(Some(i));
            }
        }
        // No job finished, report that.
        Ok(None)
    }
}

impl Default for Pool {
    /// Creates a new pool with `num_jobs` set to the number of cores.
    fn default() -> Self {
        Pool::new(0)
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        if !self.queue.is_empty() {
            panic!("dropping a non-empty process pool");
        }
    }
}

/// Error type returned by `Pool`'s methods.
#[derive(Debug)]
pub enum Error {
    /// A process finished unsuccessfully.
    CommandFailed(CommandFailed),
    /// An IO error occurred while waiting on a process.
    IoError(io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::CommandFailed(ref err) => err.fmt(f),
            Error::IoError(ref err) => err.fmt(f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::CommandFailed(ref err) => err.description(),
            Error::IoError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::CommandFailed(ref err) => Some(err),
            Error::IoError(ref err) => Some(err),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<CommandFailed> for Error {
    fn from(err: CommandFailed) -> Self {
        Error::CommandFailed(err)
    }
}
