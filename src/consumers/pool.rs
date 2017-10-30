
use std::time;
use std::thread;

use super::children::{self, RunningChild, FinishedChild};


/// Waits until there is a free tocken in the stock.
///
/// This function keeps trying to get a token from the given
/// `TokenStock`. If it cannot get one, it waits a little and reaps any
/// finished children from the given `ProcessPool` to free up tokens,
/// then tries again.
///
/// Whenever a finished child is reaped, the call-back function
/// `reaper` is called with it as an argument. This allows the caller
/// to check the freed children for errors before sending them off to
/// the nirvana. If `reaper` returns `Ok(())`, this function continues
/// normally. Otherwise, the function aborts and passes the error on.
/// (The associated token is not lost.)
///
/// # Errors
/// This function returns an error in two cases:
/// 1. If waiting on any child in the `ProcessPool` fails, this returns
///    `Err(children::Error::IoError)`;
/// 2. If `reaper` returns an error, this error is passed through.
pub fn spin_wait_for_token<F>(
    stock: &mut TokenStock,
    children: &mut ProcessPool,
    mut reaper: F,
) -> children::Result<PoolToken>
where
    F: FnMut(FinishedChild) -> children::Result<()>,
{
    loop {
        // If there are free tokens, just take one.
        if let Some(token) = stock.get_token() {
            return Ok(token);
        }
        // If not, wait a little (to go easy on the CPU) ...
        thread::sleep(time::Duration::from_millis(10));
        // ... and clear out any finished children.
        for (child, token) in children.reap() {
            stock.return_token(token);
            reaper(child?)?;
        }
    }
}

/// Tokens returned by `TokenStock`.
///
/// The only purpose of these tokens is to be handed out and redeemed.
/// This allows controlling how many jobs are running at any time.
#[derive(Debug)]
#[must_use]
pub struct PoolToken(());

/// A stock of `PoolToken`s.
///
/// This type allows predefining a set of tokens which may be given
/// out, carried around, and later redeemed. The maximum number of
/// available tokens is specified at construction and cannot be
/// changed.
///
/// `ProcessPool` limits the number of child processes that can run at
/// any time by requiring a token when accepting a new child process
/// and by only returning said token once the child has finished
/// running.
#[derive(Debug)]
pub struct TokenStock {
    /// The number of tokens remaining in this stock.
    num_tokens: usize,
}

impl TokenStock {
    /// Creates a new stock with an initial size of `max_tokens`.
    ///
    /// # Panics
    /// This panics if `max_tokens` is `0`.
    pub fn new(max_tokens: usize) -> Self {
        if max_tokens == 0 {
            panic!("invalid maximum number of tokens: 0")
        }
        Self { num_tokens: max_tokens }
    }

    /// Returns the number of currently available tokens.
    pub fn num_remaining(&self) -> usize {
        self.num_tokens
    }

    /// Returns `Some(token)` if a token is available, otherwise `None`.
    pub fn get_token(&mut self) -> Option<PoolToken> {
        if self.num_tokens > 0 {
            self.num_tokens -= 1;
            Some(PoolToken(()))
        } else {
            None
        }
    }

    /// Accepts a previously handed-out token back into the stock.
    pub fn return_token(&mut self, _: PoolToken) {
        self.num_tokens += 1;
    }
}

impl Default for TokenStock {
    /// The default for a token stock is to contain a single token.
    fn default() -> Self {
        Self::new(1)
    }
}


/// A pool of processes which can run concurrently.
///
/// Adding a new child process into this pool requires a `PoolToken`
/// handed out by `TokenStock`. This allows us to limit the number of
/// child process that can run at a time.
///
/// # Panics
/// Waiting on a child processes and even just checking whether it has
/// finished can cause a panic.
///
/// As a safety measure, `ProcessPool` also panics if it is dropped
/// while still containing child processes. You must ensure that the
/// pool is empty before leaving its scope, for example via
/// `wait_and_reap_all()`.
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

    /// Waits for all processes left in the queue to finish.
    ///
    /// This waits for all remaining children in this pool to finish
    /// running and only *then* returns an iterator over the resulting
    /// `FinishedChild`ren. Due to that, it is okay not to exhaust the
    /// returned iterator; the children will all have exited in any
    /// case.
    ///
    /// # Errors
    /// If waiting on any child fails, its respective entry in the
    /// vector will contain an `Err` instead of an `Ok`.
    pub fn wait_and_reap_all(&mut self) -> FinishedIntoIter {
        FinishedIntoIter::new(self)
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
        FinishedIter {
            queue: &mut pool.queue,
            index: 0,
        }
    }
}

impl<'a> Iterator for FinishedIter<'a> {
    type Item = (children::Result<FinishedChild>, PoolToken);

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


/// An iterator that finishes all child processes in a `ProcessPool`.
///
/// This iterator is returned by `ProcessPool::finish_all()`.
pub struct FinishedIntoIter<'a>(::std::vec::Drain<'a, RunningChild>);

impl<'a> FinishedIntoIter<'a> {
    /// Creates a new iterator that drains `pool`'s queue.
    fn new(pool: &'a mut ProcessPool) -> Self {
        // Wait for all children now so it's no problem if the caller doesn't
        // exhaust this iterator.
        for child in pool.queue.iter_mut() {
            // Ignore any errors for now, we return them via `next()`.
            let _ = child.wait();
        }
        FinishedIntoIter(pool.queue.drain(..))
    }
}

impl<'a> Iterator for FinishedIntoIter<'a> {
    type Item = (children::Result<FinishedChild>, PoolToken);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(RunningChild::finish)
    }
}
