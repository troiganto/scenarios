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


use std::fmt;
use std::mem;

use failure::{Error, Fail};
use futures::{Async, Future, Poll};

use super::children::RunningChild;


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
    children: Vec<RunningChild>,
}

impl ProcessPool {
    /// Creates a new, empty process pool of the given maximum size.
    pub fn new(capacity: usize) -> Self {
        Self {
            children: Vec::with_capacity(capacity),
        }
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
        WaitForSlot::new(&mut self.children)
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
    /// Executes the destructor for this type.
    ///
    /// The destructor panics if the pool is not empty.
    fn drop(&mut self) {
        if !self.is_empty() {
            panic!("dropping a non-empty process pool");
        }
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

impl<'a, T: 'a> WaitForSlot<'a, T> {
    /// Create a new object in the initial state.
    fn new(vec: &'a mut Vec<T>) -> Self {
        WaitForSlot::Unpolled(vec)
    }
}

impl<'a, T> Future for WaitForSlot<'a, T>
where
    T: 'a + Future,
    Error: From<T::Error>,
{
    type Item = (Slot<'a, T>, Option<T::Item>);
    type Error = WaitForSlotFailed;

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
            WaitForSlot::SlotTaken => return Err(WaitForSlotFailed::SlotTaken),
        };
        // The pool is full, check if a spot has become free.
        let async = select.poll().map_err(|err| WaitForSlotFailed::FutureFailed(err.into()))?;
        let async = match async {
            Async::Ready(result) => Async::Ready((Slot(select.0), Some(result))),
            Async::NotReady => {
                *self = WaitForSlot::Waiting(select);
                Async::NotReady
            },
        };
        Ok(async)
    }
}


/// An error occured while waiting for a slot in the process pool.
///
/// This is the error type used by [`WaitForSlot`].
///
/// [`WaitForSlot`]: ./enum.WaitForSlot.html
#[derive(Debug)]
pub enum WaitForSlotFailed {
    /// The slot has been taken by a previous call to `poll()`.
    SlotTaken,
    /// An error occured while waiting for a slot to become free.
    FutureFailed(Error),
}

impl WaitForSlotFailed {
    /// If something else has caused the error, return it.
    pub fn into_inner(self) -> Option<Error> {
        match self {
            WaitForSlotFailed::SlotTaken => None,
            WaitForSlotFailed::FutureFailed(err) => Some(err),
        }
    }
}

impl fmt::Display for WaitForSlotFailed {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            WaitForSlotFailed::SlotTaken => write!(f, "waiting for a free spot failed"),
            WaitForSlotFailed::FutureFailed(_) => write!(f, "error while waiting on child"),
        }
    }
}

impl Fail for WaitForSlotFailed {
    fn cause(&self) -> Option<&Fail> {
        match *self {
            WaitForSlotFailed::SlotTaken => None,
            WaitForSlotFailed::FutureFailed(ref err) => Some(err.cause()),
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
            .enumerate()
            .filter_map(|(i, item)| match item.poll() {
                Ok(Async::NotReady) => None,
                Ok(Async::Ready(result)) => Some((i, Ok(result))),
                Err(err) => Some((i, Err(err))),
            })
            .next();
        // If there is one, discard it and return its result.
        if let Some((index, result)) = item {
            self.0.swap_remove(index);
            result.map(Async::Ready)
        } else {
            Ok(Async::NotReady)
        }
    }
}
