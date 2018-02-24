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
    /// If it becomes ready, it returns a `Slot` that can be used to
    /// add a new child to the pool. If space has been made by waiting
    /// for another child to finish, it is also returned.
    pub fn get_slot(&mut self) -> WaitForSlot<RunningChild> {
        WaitForSlot::new(&mut self.children)
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


pub enum WaitForSlot<'a, T: 'a> {
    SlotTaken,
    Unpolled(&'a mut Vec<T>),
    Waiting(Select<'a, T>),
}

impl<'a, T: 'a> WaitForSlot<'a, T> {
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
        let future = mem::replace(self, WaitForSlot::SlotTaken);
        let mut select = match future {
            WaitForSlot::SlotTaken => return Err(WaitForSlotFailed::SlotTaken),
            WaitForSlot::Unpolled(vec) => {
                if vec.len() < vec.capacity() {
                    return Ok(Async::Ready((Slot(vec), None)));
                }
                Select(vec)
            },
            WaitForSlot::Waiting(select) => select,
        };
        let async = select.poll().map_err(Error::from).map_err(WaitForSlotFailed::FutureFailed)?;
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

#[derive(Debug)]
pub enum WaitForSlotFailed {
    SlotTaken,
    FutureFailed(Error),
}

impl WaitForSlotFailed {
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


pub struct Slot<'a, T: 'a>(&'a mut Vec<T>);

impl<'a, T: 'a> Slot<'a, T> {
    pub fn fill(self, item: T) {
        debug_assert!(self.0.len() < self.0.capacity());
        self.0.push(item);
    }
}


pub struct Select<'a, T: 'a>(&'a mut Vec<T>);

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
                self.0.swap_remove(index);
                match result {
                    Ok(result) => Ok(Async::Ready(result)),
                    Err(err) => Err(err),
                }
            },
            None => Ok(Async::NotReady),
        }
    }
}
