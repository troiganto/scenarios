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


use std::ffi::OsStr;
use std::process::Command;

use failure::{Error, ResultExt};

use scenarios::Scenario;
use trytostr::OsStrExt;

use super::Printer;
use super::children::{PreparedChild, ScenarioNotStarted};


/// The name of the environment variable to hold the scenario name.
const SCENARIOS_NAME_NAME: &'static str = "SCENARIOS_NAME";


/// Customization flags for [`CommandLine`].
///
/// [`CommandLine`]: ./struct.CommandLine.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Start child processes in a clean environment.
    ///
    /// If `true`, child processes only receive those environment
    /// variables that are defined in a scenario.
    /// If `false`, child processes inherit the environment of this
    /// process, plus the variables of the scenarios.
    ///
    /// The default is `false`.
    pub ignore_env: bool,
    /// Replace all `"{}"` in the command line with the scenario name.
    ///
    /// If `true`, use a `Printer` to insert the scenario's name into
    /// the command line when executing it.
    /// If `false`, the command line is executed as-is.
    ///
    /// The default is `true`.
    pub insert_name_in_args: bool,
    /// Define a variable "SCENARIOS_NAME".
    ///
    /// If `true`, always define an additional environment variable
    /// whose name is "SCENARIOS_NAME". This variable contains the name
    /// of the scenario in which the child process is being executed.
    ///
    /// The default is `true`.
    pub add_scenarios_name: bool,
    /// Check for previous definitions of "SCENARIOS_NAME".
    ///
    /// If `true`, it is an error to set `add_scenarios_name` to `true`
    /// *and* supply your own environment variable whose name is
    /// "SCENARIOS_NAME". If this is `false` and `add_scenarios_name`
    /// is `true`, such a variable gets silently overwritten. If
    /// `add_scenarios_name` is `false`, this option has no effect.
    ///
    /// The default is `true`.
    pub is_strict: bool,
}

impl Default for Options {
    /// Creates an `Options` value with defaults as specified above.
    fn default() -> Self {
        Self {
            ignore_env: false,
            insert_name_in_args: true,
            add_scenarios_name: true,
            is_strict: true,
        }
    }
}


/// A consumer of `Scenario`s that executes a command line in them.
///
/// This uses the variable definitions in a scenario to define
/// environment variables. In this environment, the specified command
/// line is executed. The scenario's name can be inserted into the
/// command line (by replacing all occurrences of `"{}"` with it) and
/// defined as an additional environment variable called
/// `SCENARIOS_NAME`.
///
/// The exact behavior is customized with `Options`, a set of Boolean
/// flags.
///
/// `CommandLine` is created from an iterator over any `S` that can
/// give references to `str`. It puts these objects into its own
/// backing buffer of type `Vec<S>`.
pub struct CommandLine<S: AsRef<OsStr>> {
    /// The command line containing the program and its arguments.
    command_line: Vec<S>,
    /// Flags to customize the creation of child processes.
    options: Options,
}

impl<S: AsRef<OsStr>> CommandLine<S> {
    /// Creates a new instance wrapping a command line.
    ///
    /// The iterator should yield the name of the program to execute as
    /// well as all its arguments -- in other words, the whole command
    /// line. This function returns `None` if the iterator does not
    /// yield a single element, otherwise it is `Some(CommandLine)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let line = vec!["echo", "-n", "Hello World!"];
    /// let cl = CommandLine::new(line.iter()).unwrap();
    /// assert_eq!(cl.command_line(), &line);
    ///
    /// /// The passed command line must not be empty.
    /// assert!(CommandLine::new(Vec::new()).is_none());
    /// ```
    pub fn new<I>(command_line: I) -> Option<Self>
    where
        I: IntoIterator<Item = S>,
    {
        Self::with_options(command_line, Default::default())
    }

    /// Like `new()`, but allows you to also pass `Options`.
    pub fn with_options<I>(command_line: I, options: Options) -> Option<Self>
    where
        I: IntoIterator<Item = S>,
    {
        let command_line = command_line.into_iter().collect::<Vec<_>>();
        if command_line.is_empty() {
            None
        } else {
            CommandLine { command_line, options }.into()
        }
    }

    /// Returns a shared reference to this object's `Options`.
    pub fn options(&self) -> &Options {
        &self.options
    }

    /// Returns a mutable reference to this object's `Options`.
    pub fn options_mut(&mut self) -> &mut Options {
        &mut self.options
    }

    /// Replaces this object's `Options` with new `Options`.
    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    /// Returns the full command line wrapped by this object.
    pub fn command_line(&self) -> &[S] {
        &self.command_line
    }

    /// Returns the command line split into program and its arguments.
    pub fn program_args(&self) -> (&S, &[S]) {
        self.command_line()
            .split_first()
            .expect("command line is empty")
    }

    /// Returns the name of the program to execute.
    pub fn program(&self) -> &S {
        self.program_args().0
    }

    /// Returns the arguments that `self.program()` will receive.
    pub fn args(&self) -> &[S] {
        self.program_args().1
    }

    /// Prepare an `std::process::Command` from this command line.
    ///
    /// The returned `Command` can be used to spawn a child process.
    ///
    /// # Errors
    /// This fails if strict mode is enabled and the scenario contains
    /// a variable named `"SCENARIOS_NAME"` even though this command
    /// line is instructed to add such a variable itself. (See
    /// documentation of `Options` for more information.)
    pub fn with_scenario(&self, scenario: Scenario) -> Result<PreparedChild, Error> {
        let (name, variables) = scenario.into_parts();
        let command = self.create_command(variables, &name)?;
        let program = self.program().as_ref().as_ref();
        Ok(PreparedChild::new(name.into_owned(), program, command))
    }

    /// Internal implementation of `with_scenario`.
    fn create_command<I, K, V>(&self, env_vars: I, name: &str) -> Result<Command, Error>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let mut cmd = Command::new(self.program().as_ref());
        // Go through each of the options and prepare `cmd` accordingly.
        if self.options.insert_name_in_args {
            self.add_args_formatted(&mut cmd, name)
                .context("could not replace \"{}\" with scenario name in an argument")?;
        } else {
            cmd.args(self.args().iter().map(AsRef::as_ref));
        }
        if self.options.ignore_env {
            cmd.env_clear();
        }
        if self.options.add_scenarios_name && self.options.is_strict {
            Self::add_vars_checked(&mut cmd, env_vars)
                .map_err(ReservedVarName)
                .with_context(|_| ScenarioNotStarted(name.to_owned()))?;
        } else {
            cmd.envs(env_vars);
        }
        if self.options.add_scenarios_name {
            cmd.env(SCENARIOS_NAME_NAME, OsStr::new(name));
        }
        Ok(cmd)
    }

    /// Inserts `name` into `self.args()` before adding them to `cmd`.
    fn add_args_formatted(&self, cmd: &mut Command, name: &str) -> Result<(), Error> {
        // We treat each argument as a template in which `name` is
        // inserted before being added to `cmd`.
        let mut printer = Printer::new_null();
        for arg in self.args().iter() {
            printer.set_template(arg.as_ref().try_to_str()?);
            cmd.arg(printer.format(name));
        }
        Ok(())
    }

    /// Checks the name of each variable before adding it to `cmd`.
    fn add_vars_checked<I, K, V>(cmd: &mut Command, vars: I) -> Result<(), String>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (k, v) in vars.into_iter() {
            if k.as_ref() == SCENARIOS_NAME_NAME {
                return Err(SCENARIOS_NAME_NAME.to_owned());
            }
            cmd.env(k, v);
        }
        Ok(())
    }
}


/// The error type used by `with_scenario()`.
#[derive(Debug, Fail)]
#[fail(display = "use of reserved variable name: \"{}\" (strict mode is enabled)", _0)]
pub struct ReservedVarName(String);


#[cfg(test)]
mod tests {
    use std::iter;

    use super::*;


    #[test]
    fn test_echo() {
        let cl = CommandLine::new(["echo", "-n"].iter()).unwrap();
        cl.create_command(iter::empty::<(&str, &str)>(), "name")
            .expect("CommandLine::create_command failed")
            .status()
            .expect("Child::status failed");
    }

    #[test]
    fn test_insert_name() {
        let mut cl = CommandLine::new(["echo", "a cool {}!"].iter()).unwrap();
        cl.options_mut().insert_name_in_args = true;
        let output = cl.create_command(iter::empty::<(&str, &str)>(), "name")
            .expect("CommandLine::create_command failed")
            .output()
            .expect("Child::output failed");
        let output = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output, "a cool name!\n");
    }
}
