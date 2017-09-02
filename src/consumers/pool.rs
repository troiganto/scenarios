
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
        if self.queue.len() < self.num_jobs.get() {
            let result = match command.spawn() {
                Ok(process) => {
                    self.queue.push_back(process);
                    Ok(())
                },
                Err(err) => Err(err),
            };
            PoolAddResult::CommandSpawned(result)
        } else {
            PoolAddResult::PoolFull(command)
        }
    }

    /// Like `try_push`, but may also pop a process from the pool.
    ///
    /// As long as the pool's queue is not full, this method works
    /// exactly like `try_push`.
    ///
    /// If the queue is full, this method waits for a queued process to
    /// finish. Then, the new process is added to the pool and the old
    /// process's exit status is returned.
    ///
    /// # Errors
    /// This method fails if either of the internal calls to `try_push`
    /// or `pop_finished` fails. Contrary to `try_push`, this function
    /// never returns the passed `Command`. Hence, if any error occurs,
    /// `command` is lost.
    pub fn pop_push(&mut self, command: Command) -> io::Result<Option<ExitStatus>> {
        match self.try_push(command) {
            // If we can immediately add the process to the pool, we
            // return `Ok(None)` except any io::Error occurs.
            PoolAddResult::CommandSpawned(spawn_result) => spawn_result.map(|_| None),
            // If the pool is full, we have to clear it first.
            PoolAddResult::PoolFull(command) => {
                // We try to clear out the pool. If that fails, we
                // discard the command and return the error. Because
                // the pool is full, `pop_finished()` cannot return
                // `None`.
                let exit_status = self.pop_finished().expect("no process in full pool")?;
                // Here, we know that the pool has space again, so
                // adding a new command must succeed, except any
                // `io::Error` occurs.
                self.try_push(command)
                    .expect_spawned("pool full after pop")
                    .map(|_| Some(exit_status))
            },
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
    pub fn pop_finished(&mut self) -> Option<io::Result<ExitStatus>> {
        if self.queue.is_empty() {
            return None;
        }
        let index;
        loop {
            match self.find_first_finished() {
                Ok(Some(i)) => {
                    index = i;
                    break;
                },
                Ok(None) => {
                    thread::sleep(time::Duration::from_millis(10));
                },
                Err(err) => {
                    return Some(Err(err));
                },
            }
        }
        self.queue
            .remove(index)
            .expect("index returned by `find_finished` invalid")
            .wait()
            .into()
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
