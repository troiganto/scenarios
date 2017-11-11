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


//! Module with the tiniest logger you can imagine.
//!
//! While using a crate like `slog` or `env_logger` might come first to
//! mind, even the smallest of their implementations is still way
//! overblown for this application. For example:
//! - our logging is entirely single-threaded,
//! - does not need timestamps,
//! - does not need logging levels,
//! - does not need multiple drains
//! - does not need to read config files.
//!
//! All we are interested in is printing to `stderr` unless a `quiet`
//! flag is set. Should be simple enough to roll out on our own!

use std::fmt::Display;


pub struct Logger<'a> {
    /// The name of the application. Usually set to `crate_name!()`.
    name: &'a str,
    /// If set to `true`, suppresses all output.
    quiet: bool,
}

impl Logger<'static> {
    /// Creates a logger with the default name `crate_name!()`.
    pub fn new(quiet: bool) -> Self {
        Logger::with_name(crate_name!(), quiet)
    }
}

impl<'a> Logger<'a> {
    /// Creates a logger with a custom name.
    pub fn with_name(name: &'a str, quiet: bool) -> Self {
        Logger { name, quiet }
    }

    /// Prints the given message to `stderr`.
    pub fn log<D: Display>(&self, message: D) {
        if !self.quiet {
            eprintln!("{}: {}", self.name, message);
        }
    }
}
