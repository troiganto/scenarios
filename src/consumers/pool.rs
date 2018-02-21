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


use std::thread;
use std::time;

use failure::Error;
use futures::{Async, Future, Poll};

use super::children::{FinishedChild, PreparedChild, RunningChild};
use super::tokens::{PoolToken, TokenStock};


/// A pool of processes which can run concurrently.
///
/// This is basically a vector over [`RunningChild`]ren that allows you
/// to easily check any children that have finished running and to
/// remove them from the pool.
///
/// # Panics
/// As a safety measure, this type panics if it is dropped while still
/// containing child processes. You must ensure that the pool is empty
/// before dropping it – for example by calling [`wait_reap()`] until
/// it returns `None`.
///
/// [`RunningChild`]: ./struct.RunningChild.html
/// [`wait_reap()`]: #method.wait_reap
#[derive(Debug, Default)]
pub struct ProcessPool {
    /// The list of currently running child processes.
    queue: Vec<RunningChild>,
    stock: TokenStock,
}

impl ProcessPool {
    /// Creates a new, empty process pool.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new, empty process pool with a given capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            queue: Vec::with_capacity(cap),
            stock: TokenStock::default(),
        }
    }

    /// Returns `true` if no child processes are currently in the pool.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Adds a new child process to the pool.
    pub fn try_push(
        &mut self,
        child: PreparedChild,
        token: PoolToken,
    ) -> Result<(), (Error, PoolToken)> {
        let child = match child.spawn() {
            Ok(child) => child,
            Err(err) => return Err((err, token)),
        };
        self.stock.return_token(token);
        self.queue.push(child);
        Ok(())
    }

    /// Returns an iterator over all finished child processes.
    ///
    /// Each call to `FinishedIter::next()` removes
    /// `Some((child, token))` from the pool. If all child processes
    /// are still running, the returned iterator is empty.
    pub fn reap(&mut self) -> FinishedIter {
        FinishedIter::new(self)
    }

    /// Waits for any child process to finish.
    ///
    /// This call blocks until at least one child process is finished
    /// and then returns that process. If the pool is empty, this
    /// function returns `None`.
    ///
    /// # Errors
    /// If waiting on any child fails, this function returns the error
    /// that occurred.
    pub fn wait_reap(&mut self) -> Option<(Result<FinishedChild, Error>, PoolToken)> {
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
    /// Executes the destructor for this type.
    ///
    /// The destructor panics if the pool is not empty.
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
    pool: &'a mut ProcessPool,
    /// The current iteration index.
    index: usize,
}

impl<'a> FinishedIter<'a> {
    /// Creates a new iterator that borrows `pool`'s queue.
    fn new(pool: &'a mut ProcessPool) -> Self {
        let index = 0;
        FinishedIter { pool, index }
    }
}

impl<'a> Iterator for FinishedIter<'a> {
    type Item = (Result<FinishedChild, Error>, PoolToken);

    fn next(&mut self) -> Option<Self::Item> {
        // Iterate until we've traversed the entire vector.
        while self.index < self.pool.queue.len() {
            let is_finished = self.pool.queue[self.index].check_finished();
            match is_finished {
                // No matter whether the child is finished or waiting
                // on it gives an error -- we eject it in both cases.
                // (Note: This assumes that waiting twice on the same
                // child gives the same error.)
                Ok(true) | Err(_) => {
                    let child = self.pool.queue.swap_remove(self.index);
                    let token = self.pool.stock.get_token().unwrap();
                    return Some((child.finish(), token));
                },
                Ok(false) => {
                    self.index += 1;
                },
            }
        }
        None
    }
}


pub struct LimitedVec<T>(Vec<T>);

impl<T> LimitedVec<T> {
    pub fn new(size: usize) -> Self {
        LimitedVec(Vec::with_capacity(size))
    }

    pub fn max_len(&self) -> usize {
        self.0.capacity()
    }

    pub fn is_full(&self) -> bool {
        self.len() >= self.max_len()
    }

    pub fn into_inner(self) -> Vec<T> {
        self.0
    }

    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.0.as_mut_slice()
    }

    pub fn iter(&self) -> ::std::slice::Iter<T> {
        self.as_slice().iter()
    }

    pub fn iter_mut(&mut self) -> ::std::slice::IterMut<T> {
        self.as_mut_slice().iter_mut()
    }

    pub fn try_push(&mut self, item: T) -> Result<(), T> {
        if self.len() < self.max_len() {
            self.0.push(item);
            Ok(())
        } else {
            Err(item)
        }
    }

    pub fn force_push(&mut self, item: T) {
        assert!(self.try_push(item).is_ok(), "limited vec is full");
    }

    pub fn try_push_from(&mut self, item: &mut Option<T>) {
        if self.len() < self.max_len() {
            self.0.push(item.take().unwrap());
        }
    }

    pub fn remove(&mut self, index: usize) -> T {
        self.0.remove(index)
    }

    pub fn select(&mut self) -> Select<T> {
        assert!(!self.is_empty());
        Select(self)
    }
}

impl<T> IntoIterator for LimitedVec<T> {
    type Item = T;
    type IntoIter = ::std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a LimitedVec<T> {
    type Item = &'a T;
    type IntoIter = ::std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a, T> IntoIterator for &'a mut LimitedVec<T> {
    type Item = &'a mut T;
    type IntoIter = ::std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T> ::std::ops::Deref for LimitedVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T> ::std::ops::DerefMut for LimitedVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}


pub struct Select<'a, T: 'a>(&'a mut LimitedVec<T>);

impl<'a, T> Future for Select<'a, T>
where
    T: 'a + Future,
{
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let item = self.0
            .iter_mut()
            .enumerate()
            .filter_map(|(i, item)| match item.poll() {
                Ok(Async::NotReady) => None,
                Ok(Async::Ready(result)) => Some((i, Ok(result))),
                Err(err) => Some((i, Err(err))),
            })
            .next();
        match item {
            Some((index, result)) => {
                self.0.remove(index);
                match result {
                    Ok(result) => Ok(Async::Ready(result)),
                    Err(err) => Err(err),
                }
            },
            None => Ok(Async::NotReady),
        }
    }
}
