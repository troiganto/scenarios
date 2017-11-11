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


/// Tokens returned by `TokenStock`.
///
/// The only purpose of these tokens is to be handed out and redeemed.
/// This allows controlling how many jobs are running at any time.
#[derive(Debug)]
#[must_use]
pub struct PoolToken(());
