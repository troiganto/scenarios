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


#![allow(dead_code)]

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};


/// The name of the executable being tested.
#[cfg(windows)]
static PROGRAM_NAME: &str = "scenarios.exe";

/// The name of the executable being tested.
#[cfg(not(windows))]
static PROGRAM_NAME: &str = "scenarios";


/// A type that helps us execute our program and check the output.
pub struct Runner {
    command: Command,
    tests_dir: PathBuf,
}

impl Runner {
    /// Creates a new runner.
    ///
    /// # Panics
    /// This tries to figure out where our executable and our test
    /// scenarios files are. If it can't find both of them, this
    /// function panics.
    pub fn new() -> Self {
        Runner {
            command: Command::new(guess_bin_path()),
            tests_dir: guess_tests_dir_path(),
        }
    }

    /// Adds an argument to pass to the program.
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Self {
        self.command.arg(arg);
        self
    }

    /// Adds multiple arguments to pass to the program.
    pub fn args<I>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        self.command.args(args);
        self
    }

    /// Returns the path of the given example scenario file.
    ///
    /// # Panics
    /// This panics if the file cannot be found, or if `filename` is
    /// not a relative path.
    pub fn get_scenario_file_path<S: AsRef<Path>>(&self, filename: S) -> PathBuf {
        if !filename.as_ref().is_relative() {
            panic!("not a relative path: {}", filename.as_ref().display());
        }
        let mut path = self.tests_dir.clone();
        path.push(filename);
        if !path.is_file() {
            panic!("not a file: {}", path.display());
        }
        path
    }

    /// Adds a scenario file to pass as an argument.
    ///
    /// This is like `arg()`, except it automatically prepends the
    /// directory of example scenario files to `filename`.
    ///
    /// # Panics
    /// This panics if `filename` is not a relative path, or if the
    /// file cannot be found.
    pub fn scenario_file<S: AsRef<Path>>(&mut self, filename: S) -> &mut Self {
        let path = self.get_scenario_file_path(filename);
        self.arg(path)
    }

    /// This is to `scenario_file` what `args` is to `arg`.
    pub fn scenario_files<I>(&mut self, filenames: I) -> &mut Self
    where
        I: IntoIterator,
        I::Item: AsRef<Path>,
    {
        for filename in filenames {
            self.scenario_file(filename);
        }
        self
    }

    /// Runs the command and returns its output.
    pub fn output(&mut self) -> RunResult {
        let output = self.command
            .env_clear()
            .env("outer_variable", "1")
            .output()
            .expect("could not spawn");
        RunResult::new(output)
    }
}

impl Default for Runner {
    fn default() -> Self {
        Self::new()
    }
}


/// The type returned by `Runner::output()`.
///
/// This type is a replacement of `std::process::Output` that
/// automatically converts stdout and stderr to `String`.
pub struct RunResult {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl RunResult {
    fn new(output: Output) -> Self {
        RunResult {
            status: output.status,
            stdout: String::from_utf8(output.stdout).expect("stdout is not utf8"),
            stderr: String::from_utf8(output.stderr).expect("stderr is not utf8"),
        }
    }
}


/// Extension trait for `OsString` providing conversion from `&[u8]`.
pub trait OsStringExt {
    fn from_bytes(bytes: &[u8]) -> Self;
}

impl OsStringExt for OsString {
    #[cfg(unix)]
    fn from_bytes(bytes: &[u8]) -> OsString {
        use std::os::unix::ffi::OsStringExt;

        OsString::from_vec(bytes.to_owned())
    }

    #[cfg(windows)]
    fn from_bytes(bytes: &[u8]) -> OsString {
        use std::os::windows::ffi::OsStringExt;

        let wide = bytes.map(|b| b as u16).collect::<Vec<u16>>();
        OsString::from_wide(&wide)
    }

    #[cfg(not(any(unix, windows)))]
    fn from_bytes(bytes: &[u8]) -> OsString {
        unsafe { OsString::from(String::from_utf8_unchecked(bytes.to_owned())) }
    }
}


fn guess_tests_dir_path() -> PathBuf {
    // We pray to Ferris that the current working directory always is the
    // root of our project.
    let mut tests_dir = env::current_dir().expect("could not get current directory");
    tests_dir.push("tests");
    if tests_dir.is_dir() {
        return tests_dir;
    }
    panic!("could not find test files for `scenarios`");
}


fn guess_bin_path() -> PathBuf {
    let mut executable = env::current_exe().expect("could not get current executable");
    // First, we check the directory of the test executable.
    executable.pop();
    executable.push(PROGRAM_NAME);
    if executable.is_file() {
        return executable;
    }
    // Then, we check the parent directory.
    executable.pop();
    executable.pop();
    executable.push(PROGRAM_NAME);
    if executable.is_file() {
        return executable;
    }
    // Then, we give up.
    panic!("could not find executable for `scenarios`");
}
