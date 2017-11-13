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


use std::time;
use std::thread;

use super::errors::Result;
use super::tokens::PoolToken;
use super::children::{RunningChild, FinishedChild};


/// A pool of processes which can run concurrently.
///
/// This is basically a vector over `RunningChild`ren that allows you
/// to easily check any children that have finished running and to
/// remove them from the pool.
///
/// # Panics
/// As a safety measure, `ProcessPool` panics if it is dropped while
/// still containing child processes. You must ensure that the pool is
/// empty before dropping it -- for example by calling `wait_reap()`
/// until it returns `None`.
#[derive(Debug, Default)]
pub struct ProcessPool {
    /// The list of currently running child processes.
    queue: Vec<RunningChild>,
}

impl ProcessPool {
    /// Creates a new, empty process pool.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new, empty process pool with a given capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self { queue: Vec::with_capacity(cap) }
    }

    /// Returns `true` if no child processes are currently in the pool.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Adds a new child process to the pool.
    pub fn push(&mut self, child: RunningChild) {
        self.queue.push(child)
    }

    /// Returns an iterator over all finished child processes.
    ///
    /// Each call to `FinishedIter::next` removes `Some((child, token))`
    /// from the pool. If all child processes are still running, the
    /// call to `next` returns `None`.
    pub fn reap(&mut self) -> FinishedIter {
        FinishedIter::new(self)
    }

    /// Waits for any child process to finish.
    ///
    /// This loops around until at least one child process is finished
    /// and returns that process. This only returns `None` if the pool
    /// is completely empty.
    ///
    /// # Errors
    /// If waiting on any child fails, this function returns
    /// `Some((Err(_), token))`.
    pub fn wait_reap(&mut self) -> Option<(Result<FinishedChild>, PoolToken)> {
        if self.is_empty() {
            return None;
        }
        loop {
            // `child` is only `None` if *no* child has finished. Wait.
            let child = self.reap().next();
            if child.is_some() {
                return child;
            }
            thread::sleep(time::Duration::from_millis(10));
        }
    }
}

impl Drop for ProcessPool {
    fn drop(&mut self) {
        if !self.is_empty() {
            panic!("dropping a non-empty process pool");
        }
    }
}


/// An iterator over the finished child processes in a `ProcessPool`.
///
/// This iterator is returned by `ProcessPool::reap()`.
pub struct FinishedIter<'a> {
    /// The borrowed queue of child processes.
    queue: &'a mut Vec<RunningChild>,
    /// The current iteration index.
    index: usize,
}

impl<'a> FinishedIter<'a> {
    /// Creates a new iterator that borrows `pool`'s queue.
    fn new(pool: &'a mut ProcessPool) -> Self {
        FinishedIter { queue: &mut pool.queue, index: 0 }
    }
}

impl<'a> Iterator for FinishedIter<'a> {
    type Item = (Result<FinishedChild>, PoolToken);

    fn next(&mut self) -> Option<Self::Item> {
        // Iterate until we've traversed the entire vector.
        while self.index < self.queue.len() {
            let is_finished = self.queue[self.index].is_finished();
            match is_finished {
                // No matter whether the child is finished or waiting
                // on it gives an error -- we eject it in both cases.
                // (Note: This assumes that waiting twice on the same
                // child gives the same error.)
                Ok(true) | Err(_) => {
                    let child = self.queue.swap_remove(self.index);
                    return Some(child.finish());
                },
                Ok(false) => {
                    self.index += 1;
                },
            }
        }
        None
    }
}
