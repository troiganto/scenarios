
use std::io;
use std::process::{Child, Command, ExitStatus};
use std::collections::VecDeque;
use std::thread;
use std::time;

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

    /// Returns `true` if the pool is full.
    ///
    /// If this function returns `true`, the next call to `try_push`
    /// will return `PoolAddResult::PoolFull`.
    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.num_jobs.get()
    }

    /// Adds a new process to the pool.
    ///
    /// If the pool is full, this call fails and returns the passed
    /// `command`. If the pool is not full, a new child process is
    /// spawned from the `command` and added to the pool.
    ///
    /// # Errors
    /// If the pool is full, the passed `command` is returned, wrapped
    /// in a `PoolAddResult::PoolFull`.
    ///
    /// If the pool is not full and spawning the child process fails,
    /// `PoolAddResult::CommandSpawned(Err(error))` is returned. The
    /// pool is not modified in this case.
    ///
    /// If spawning the child process, succeeds,
    /// `PoolAddResult::CommandSpawned(Ok(()))` is returned.
    pub fn try_push(&mut self, mut command: Command) -> PoolAddResult {
        if self.is_full() {
            PoolAddResult::PoolFull(command)
        } else {
            let result = command
                .spawn()
                .map(|process| { self.queue.push_back(process); });
            PoolAddResult::CommandSpawned(result)
        }
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
    pub fn join(&mut self) -> io::Result<Vec<ExitStatus>> {
        self.queue
            .drain(..)
            .map(|mut job| job.wait())
            .collect()
    }

    /// Like pop_finished(), but returns `None` if the pool is not
    /// completely full.
    pub fn pop_finished_if_full(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.is_full() {
            self.pop_finished()
        } else {
            Ok(None)
        }
    }

    /// Finds the first finished child process in the queue.
    ///
    /// If the queue is empty, this method just returns `None`.
    ///
    /// Otherwise, this method sequentially tries to wait for all
    /// queued processes. Once a process has finished, it is
    /// removed from the queue and its exit status is returned.
    ///
    /// # Errors
    /// This call returns `Some(Err(error))` if waiting on any process
    /// fails. The failing process may or may not be left in the queue.
    pub fn pop_finished(&mut self) -> io::Result<Option<ExitStatus>> {
        if self.queue.is_empty() {
            return Ok(None);
        }
        let index;
        loop {
            match self.find_first_finished()? {
                Some(i) => {
                    index = i;
                    break;
                },
                None => {
                    thread::sleep(time::Duration::from_millis(10));
                },
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
    fn find_first_finished(&mut self) -> io::Result<Option<usize>> {
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


/// Result-like type returned by `Pool::add()`.
pub enum PoolAddResult {
    /// The command could not be spawned because the pool is full.
    PoolFull(Command),
    /// The command was spawned, maybe even successfully.
    CommandSpawned(io::Result<()>),
}

impl PoolAddResult {
    /// Expects that the result of `Pool::add()` was `CommandSpawned`.
    ///
    /// This panics with a custom message if the command was not
    /// spawned.
    pub fn expect_spawned(self, err_msg: &'static str) -> io::Result<()> {
        match self {
            PoolAddResult::CommandSpawned(result) => result,
            _ => panic!(err_msg),
        }
    }
}
