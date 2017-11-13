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


use std::io;
use std::fmt;
use std::process::ExitStatus;
use std::error::Error as StdError;

use quick_error::Context;


/// Convenience alias for `std::result::Result`.
pub type Result<T> = ::std::result::Result<T, Error>;


/// Wrapper type that tags an `io::Error` as coming from a call to
/// `spawn()`.
///
/// This disambiguates the conversion from `io::Error` to `ErrorKind`.
#[derive(Debug)]
pub(super) struct SpawnErrorTag(pub io::Error);


/// Wrapper type that tags an `io::Error` as coming from a call to
/// `wait()`.
///
/// This disambiguates the conversion from `io::Error` to `ErrorKind`.
#[derive(Debug)]
pub(super) struct WaitErrorTag(pub io::Error);


/// The error type used by this module.
///
/// This type essentially ties a regular error (contained in
/// `ErrorKind`) together with the name of the child that caused it.
#[derive(Debug)]
pub struct Error {
    name: String,
    kind: ErrorKind,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: \"{}\"", self.description(), self.name)
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        "scenario did not finish successfully"
    }

    fn cause(&self) -> Option<&StdError> {
        Some(&self.kind)
    }
}

/// Adds scenario name as context to `Error`.
///
/// This is the simplest way to create an `Error` via quick-error's
/// `Context` struct.
impl<'a, E: Into<ErrorKind>> From<Context<&'a str, E>> for Error {
    fn from(context: Context<&'a str, E>) -> Self {
        let name = context.0.to_owned();
        let kind = context.1.into();
        Error { name, kind }
    }
}

/// Adds program and scenario name as context to `Error`.
///
/// Because `name` is a field of `Error`, but `program` is a field of
/// `ErrorKind`, it is difficult to pass both via `Context`. This
/// implementation abstracts that difficult fact away by accepting a
/// tuple as a context.
impl<Name, Program, RawError> From<Context<(Name, Program), RawError>> for Error
where
    Name: AsRef<str>,
    Program: AsRef<str>,
    ErrorKind: From<Context<Program, RawError>>
{
    fn from(context: Context<(Name, Program), RawError>) -> Self {
        let (name, program) = context.0;
        let raw_error = context.1;
        let kind = ErrorKind::from(Context(program, raw_error));
        Error { name: name.as_ref().to_owned(), kind }
    }
}


quick_error! {
    /// The kinds of errors that can be caused in this module.
    #[derive(Debug)]
    pub enum ErrorKind {
        SpawnError(program: String, err: io::Error) {
            description("could not execute command")
            display(self_) -> ("{}: {}", self_.description(), program)
            cause(err)
            context(program: AsRef<str>, err: SpawnErrorTag)
                -> (program.as_ref().to_owned(), err.0)
        }
        WaitError(program: String, err: io::Error) {
            description("error while waiting for job to finish")
            display(self_) -> ("{}: {}", self_.description(), program)
            cause(err)
            context(program: AsRef<str>, err: WaitErrorTag) -> (program.as_ref().to_owned(), err.0)
        }
        ChildFailed(program: String, status: ExitStatus) {
            description("command returned non-zero exit status")
            display("command \"{}\" exited with {}", program, status)
            context(program: AsRef<str>, status: ExitStatus)
                -> (program.as_ref().to_owned(), status)
        }
        VariableNameError(varname: String) {
            description("use of reserved variable name")
            display(self_) -> ("{}: \"{}\" (strict mode is enabled)", self_.description(), varname)
        }
    }
}
