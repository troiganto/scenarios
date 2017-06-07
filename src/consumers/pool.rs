
use std::io;
use std::error::Error as StdError;
use std::process::{Child, ExitStatus};
use std::fmt::{self, Display};
use std::collections::VecDeque;

use num_cpus;


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
/// The pool is used by continously `add`ing newly-started processes to
/// it. The `add()` call will block if the maximum number of concurrent
/// processes has been reached. Once all processes have been submitted
/// you should call `join()` to wait until they have all finished.
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
    /// This blockingly adds `process` to the pool of waited-on
    /// processes. If the pool is full, i.e. has reached `num_jobs`
    /// elements, this call waits until another process has finished.
    ///
    /// # Errors
    /// If the pool is not full, it does not block and always succeeds.
    ///
    /// If a waited-on process fails, this call fails with
    /// `Error::CommandFailed`. A waited-on process fails e.g. by
    /// returning a non-zero exit status or by aborting through a
    /// signal.
    ///
    /// Even if an error occurs, the passed process is added to the
    /// queue in any case.
    pub fn add(&mut self, process: Child) -> Result<(), Error> {
        let mut result = Ok(());
        // If the queue is full, block until one job is finished.
        if self.queue.len() == self.num_jobs.get() {
            let oldest_job = self.queue.pop_front().expect("pop from empty queue");
            result = Pool::wait_for_job(oldest_job);
        }
        self.queue.push_back(process);
        result
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
        for job in self.queue.drain(..) {
            result = result.and(Pool::wait_for_job(job));
        }
        result
    }

    /// Implementation of the waiting behavior of `join()` and `add()`.
    fn wait_for_job(mut job: Child) -> Result<(), Error> {
        let exit_status = job.wait()?;
        if exit_status.success() {
            Ok(())
        } else {
            Err(Error::CommandFailed(exit_status))
        }
    }
}

impl Default for Pool {
    /// Creates a new pool with `num_jobs` set to the number of cores.
    fn default() -> Self {
        Pool::new(0)
    }
}


/// Error type returned by `Pool`'s methods.
#[derive(Debug)]
pub enum Error {
    /// A process finished unsuccessfully.
    CommandFailed(ExitStatus),
    /// An IO error occurred while waiting on a process.
    IoError(io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::CommandFailed(ref code) => write!(f, "{}: {}", self.description(), code),
            Error::IoError(ref err) => err.fmt(f),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::CommandFailed(_) => "command returned non-zero exit code",
            Error::IoError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::CommandFailed(_) => None,
            Error::IoError(ref err) => Some(err),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}
