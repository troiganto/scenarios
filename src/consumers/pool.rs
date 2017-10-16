
use std::process::{Child, ExitStatus};


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
/// pool is empty before leaving its scope.
#[derive(Debug, Default)]
pub struct ProcessPool {
    /// The list of currently running child processes.
    queue: Vec<(Child, PoolToken)>,
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
    pub fn push(&mut self, child: Child, token: PoolToken) {
        self.queue.push((child, token))
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
    /// The returned list contains the `ExitStatus` and `PoolToken` of
    /// all remaining child processes.
    ///
    /// # Panics
    /// This panics if any IO error occurs while waiting on a child.
    pub fn join_all(&mut self) -> Vec<(ExitStatus, PoolToken)> {
        // TODO: Find a way to avoid panics.
        self.queue
            .drain(..)
            .map(|(child, token)| (wait_unwrap(child), token))
            .collect()
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
/// # Panics
/// The `next` method calls `std::process::Child::try_wait` and unwraps
/// the result. Thus, any call to `next` can, in theory, panic.
pub struct FinishedIter<'a> {
    /// The borrowed queue of child processes.
    queue: &'a mut Vec<(Child, PoolToken)>,
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
    type Item = (ExitStatus, PoolToken);

    fn next(&mut self) -> Option<Self::Item> {
        // Iterate until we've traversed the entire vector.
        while self.index < self.queue.len() {
            // The separate scope limits the borrow of `child` while we
            // check whether `child` has finished.
            let is_finished = {
                let (ref mut child, _) = self.queue[self.index];
                is_finished(child)
            };
            // If `child` _is_ finished, we remove it from the queue and
            // return it. The hole is filled up by the last element of
            // the vector. We thus leave `index` unchanged.
            if is_finished {
                let (child, token) = self.queue.swap_remove(self.index);
                return Some((wait_unwrap(child), token));
            } else {
                self.index += 1;
            }
        }
        None
    }
}


/// Calls `child.wait()` and unwraps the result.
///
/// # Panics
/// This panics if any IO error occurs while waiting.
fn wait_unwrap(mut child: Child) -> ExitStatus {
    child
        .wait()
        .expect("I/O error while waiting on child process")
}


/// Returns `true` if the `child` has finished running.
///
/// # Panics
/// This unwraps the result of `child.try_wait` and thus may panic.
fn is_finished(child: &mut Child) -> bool {
    child
        .try_wait()
        .expect("I/O error while querying child process status")
        .is_some()
}
