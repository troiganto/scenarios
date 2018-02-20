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


use failure::Error;

use super::children::FinishedChild;
use super::children::PreparedChild;
use super::pool::ProcessPool;
use super::tokens::TokenStock;


/// The interface used by [`loop_in_process_pool()`] for callbacks.
///
/// In order to call [`loop_in_process_pool()`], a type must be passed
/// that implements this trait. This trait is then used for various
/// callbacks during the actual loop.
///
/// By returning an error, the implementor is able to abort the loop at
/// any time. Nonetheless, all running child processes are always
/// waited for.
///
/// [`loop_in_process_pool()`]: ./fn.loop_in_process_pool.html
pub trait LoopDriver<Item> {
    /// Returns the number of children allowed to run in parallel.
    fn max_num_of_children(&self) -> usize;

    /// Takes some item and creates a [`PreparedChild`] from it.
    ///
    /// Beside the loop driver, an iterator is passed to the function
    /// [`loop_in_process_pool()`]. It is the task of this function to
    /// convert the items yielded by the iterator to
    /// [`PreparedChild`]ren. If this isn't possible, an error should
    /// be returned, which aborts the loop.
    ///
    /// [`PreparedChild`]: ./struct.PreparedChild.html
    /// [`loop_in_process_pool()`]: ./fn.loop_in_process_pool.html
    fn prepare_child(&self, item: Item) -> Result<PreparedChild, Error>;

    /// Handles any child processes that have terminated.
    ///
    /// This allows the implementor to e.g. check the exit status of
    /// the terminated process. If everything is alright, this function
    /// should return `Ok(())`. If the loop should be aborted, this
    /// function should return an error.
    fn on_reap(&mut self, child: FinishedChild) -> Result<(), Error>;

    /// Observes whether the loop terminated successfully.
    ///
    /// This function is called if the loop was exited not through
    /// exhaustion of the iterator that was passed to
    /// [`loop_in_process_pool()`] but because of any error.
    ///
    /// If the implementor wants `error` to be the result of
    /// [`loop_in_process_pool()`], it must hold onto it and return it
    /// later from [`on_finish()`].
    ///
    /// [`loop_in_process_pool()`]: ./fn.loop_in_process_pool.html
    /// [`on_finish()`]: #tymethod.on_finish
    fn on_loop_failed(&mut self, error: Error);

    /// Like [`on_reap()`] but called by the clean-up loop.
    ///
    /// This call-back for terminated processes is chosen if an error
    /// has occured and the loop has been aborted. Because an error is
    /// already being processed, this function is not allowed to return
    /// another error.
    ///
    /// This function should not panic because that would lead to
    /// [`ProcessPool`] being dropped while still containing running
    /// child processes, which would lead to a double panic, which
    /// would terminate the entire program.
    ///
    /// [`on_reap()`]: #tymethod.on_reap
    /// [`ProcessPool`]: ./struct.ProcessPool.html
    fn on_cleanup_reap(&mut self, child: Result<FinishedChild, Error>);

    /// Wraps up the loop after everything else has run.
    ///
    /// This function determines the result of the over-all call to
    /// [`loop_in_process_pool()`]. It gives the driver a chance to
    /// e.g. pop any errors it has previously pushed out of the way.
    ///
    /// [`loop_in_process_pool()`]: ./fn.loop_in_process_pool.html
    fn on_finish(self) -> Result<(), Error>;
}


/// Handle items from an iterator, starting a child process for each.
///
/// This goes through the `items` and starts one child process for each
/// of them. The [`PoolToken`] mechanism limits the number of processes
/// that can run at any time. A [`LoopDriver`] type is used to drive
/// the loop and answer callbacks.
///
/// # Errors
///
/// If any error occurs, the loop is exited immediately. However, all
/// child processes are still properly waited for before this function
/// returns.
///
/// This function exits with an error if:
///
/// - spawning a child process fails;
/// - waiting on a child process fails;
/// - any one of the calls to the [`LoopDriver`] fails.
///
/// [`PoolToken`]: ./struct.PoolToken.html
/// [`LoopDriver`]: ./trait.LoopDriver.html
pub fn loop_in_process_pool<I, D>(items: I, mut driver: D) -> Result<(), Error>
where
    I: IntoIterator,
    D: LoopDriver<I::Item>,
{
    // Initialize the control structures.
    let mut stock = TokenStock::new(driver.max_num_of_children());
    let mut pool = ProcessPool::new();
    // Perform the actual loop.
    let loop_result = loop_inner(&mut stock, &mut pool, items, &mut driver);
    if let Err(err) = loop_result {
        driver.on_loop_failed(err);
    }
    // If any children are left, wait for them.
    while let Some((child, _)) = pool.wait_reap() {
        driver.on_cleanup_reap(child);
    }
    driver.on_finish()
}


/// The actual main loop of [`loop_in_process_pool()`].
///
/// If no error occurs, this function waits for all child processes to
/// terminate. As soon as an error occurs, this function returns.
/// Cleaning up the pool is left to the caller in that case.
///
/// # Errors
/// Same as for [`loop_in_process_pool()`].
///
/// [`loop_in_process_pool()`]: ./fn.loop_in_process_pool.html
fn loop_inner<I, D>(
    stock: &mut TokenStock,
    pool: &mut ProcessPool,
    items: I,
    driver: &mut D,
) -> Result<(), Error>
where
    I: IntoIterator,
    D: LoopDriver<I::Item>,
{
    for item in items {
        // Get a token from the stock. If there are none left, wait for a child
        // to finish and take its token.
        let token = if let Some(token) = stock.get_token() {
            token
        } else {
            // This `unwrap()` is safe because otherwise, that would mean there are
            // no tokens at all.
            let (finished_child, token) = pool.wait_reap().unwrap();
            let finished_child = finished_child?;
            driver.on_reap(finished_child)?;
            token
        };
        // Start a new child process.
        let prepared_child = driver.prepare_child(item)?;
        pool.try_push(prepared_child, token)
            .map_err(|(err, token)| {
                stock.return_token(token);
                err
            })?;
    }
    // If nothing has gone wrong until now, we wait for all child processes
    // to terminate.
    while let Some((finished_child, _)) = pool.wait_reap() {
        let finished_child = finished_child?;
        driver.on_reap(finished_child)?;
    }
    Ok(())
}
