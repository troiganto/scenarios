#![allow(dead_code)]

use std::env;
use std::ffi::OsStr;
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


    /// Adds a scenario file to pass as an argument.
    ///
    /// This is like `arg()`, except it automatically prepends the
    /// directory of scenario files to `filename`.
    ///
    /// # Panics
    /// This panics if `filename` is not a relative path.
    pub fn scenario_file<S: AsRef<Path>>(&mut self, filename: S) -> &mut Self {
        if !filename.as_ref().is_relative() {
            panic!("not a relative path: {}", filename.as_ref().display());
        }
        let mut path = self.tests_dir.clone();
        path.push(filename);
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
            .output()
            .expect("could not spawn");
        RunResult::new(output)
    }
}


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
