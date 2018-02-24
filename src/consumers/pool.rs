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


use std::mem;

use failure;
use futures::{Async, Future, Poll};

use super::children::RunningChild;


/// A pool of processes which can run concurrently.
///
/// This is basically a vector over [`RunningChild`]ren that allows you
/// to easily check any children that have finished running and to
/// remove them from the pool.
///
/// # Panics
/// In debug mode, this type panics if it is dropped while still
/// containing child processes. In release mode, any remaining child
/// processes are killed. It is highly advisable to empty the pool
/// before dropping it.
///
/// [`RunningChild`]: ./struct.RunningChild.html
/// [`wait_reap()`]: #method.wait_reap
#[derive(Debug, Default)]
pub struct ProcessPool {
    /// The list of currently running child processes.
    children: Vec<RunningChild>,
}

impl ProcessPool {
    /// Creates a new, empty process pool of the given maximum size.
    pub fn new(capacity: usize) -> Self {
        let children = Vec::with_capacity(capacity);
        Self { children }
    }

    /// Returns `true` if no child processes are currently in the pool.
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    /// Adds a new child process to the pool, if possible.
    ///
    /// The returned future is not-ready as long as the pool is full.
    /// When it becomes ready, it returns a [`Slot`] that can be used
    /// to add a new child to the pool. If the slot has become
    /// available because another child finished running, the
    /// [`FinishedChild`] is returned as well.
    ///
    /// # Errors
    ///
    /// Waiting on a child may fail. This is highly dependent on the
    /// platform you are running on. If waiting on a child fails, no
    /// slot is returned, but the child is still removed from the pool.
    /// You may call this function again after handling the error and
    /// get a free slot immediately.
    ///
    /// [`Slot`]: ./struct.Slot.html
    /// [`FinishedChild`]: ./struct.FinishedChild.html
    pub fn get_slot(&mut self) -> WaitForSlot<RunningChild> {
        WaitForSlot::Unpolled(&mut self.children)
    }

    /// Returns one finished child.
    ///
    /// The returned future is not-ready until at least one child in
    /// this pool finishes running. When it becomes ready, the
    /// [`FinishedChild`] is returned.
    ///
    /// # Errors
    ///
    /// Waiting on a child may fail. This is highly dependent on the
    /// platform you are running on. If waiting on a child fails, the
    /// child is still removed from the pool.
    ///
    /// [`FinishedChild`]: ./struct.FinishedChild.html
    pub fn reap_one(&mut self) -> Select<RunningChild> {
        Select(&mut self.children)
    }
}

impl Drop for ProcessPool {
    fn drop(&mut self) {
        debug_assert!(self.is_empty(), "dropping a non-empty process pool");
    }
}


/// Future returned by [`ProcessPool::get_slot()`].
///
/// [`ProcessPool::get_slot()`]: ./struct.ProcessPool.html#method.get_slot
pub enum WaitForSlot<'a, T: 'a> {
    /// Initial state.
    Unpolled(&'a mut Vec<T>),
    /// The pool is full and we are waiting on a spot to become free.
    Waiting(Select<'a, T>),
    /// The future has finished and will never give a slot again.
    SlotTaken,
}

impl<'a, T> Future for WaitForSlot<'a, T>
where
    T: 'a + Future,
    failure::Error: From<T::Error>,
{
    type Item = (Slot<'a, T>, Option<T::Item>);
    type Error = failure::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // Set the future to a dummy state while we're processing it.
        let future = mem::replace(self, WaitForSlot::SlotTaken);
        let mut select = match future {
            WaitForSlot::Unpolled(vec) => {
                if vec.len() < vec.capacity() {
                    return Ok(Async::Ready((Slot(vec), None)));
                }
                Select(vec)
            },
            WaitForSlot::Waiting(select) => select,
            WaitForSlot::SlotTaken => panic!("slot already taken"),
        };
        // The pool is full, check if a spot has become free.
        match select.poll()? {
            Async::Ready(result) => Ok(Async::Ready((Slot(select.0), Some(result)))),
            Async::NotReady => {
                *self = WaitForSlot::Waiting(select);
                Ok(Async::NotReady)
            },
        }
    }
}


/// Type representing an available spot in a [`ProcessPool`].
///
/// This type ensures that, even in the face of errors, the process
/// pool can never grow beyond its capacity.
///
/// [`ProcessPool`]: ./struct.ProcessPool.html
pub struct Slot<'a, T: 'a>(&'a mut Vec<T>);

impl<'a, T: 'a> Slot<'a, T> {
    /// Fills the slot by pushing an item to the queue.
    pub fn fill(self, item: T) {
        debug_assert!(self.0.len() < self.0.capacity());
        self.0.push(item);
    }
}


/// Future returned by [`ProcessPool::reap_one()`].
///
/// [`ProcessPool::reap_one()`]: ./struct.ProcessPool.html#method.reap_one
pub struct Select<'a, T: 'a>(&'a mut Vec<T>);

impl<'a, T> Future for Select<'a, T>
where
    T: 'a + Future,
{
    type Item = T::Item;
    type Error = T::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // Find the first future that has become ready.
        let item = self.0
            .iter_mut()
            .map(Future::poll)
            .enumerate()
            .find(|&(_, ref poll)| is_ready_or_err(poll));
        // If there is one, discard it and return its result.
        if let Some((index, result)) = item {
            self.0.swap_remove(index);
            result
        } else {
            Ok(Async::NotReady)
        }
    }
}


/// Returns `true` if a `poll` indicates that its future has finished.
fn is_ready_or_err<T, E>(poll: &Poll<T, E>) -> bool {
    match *poll {
        Ok(Async::Ready(_)) | Err(_) => true,
        Ok(Async::NotReady) => false,
    }
}
