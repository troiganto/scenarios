
use std::ffi::OsStr;
use std::error::Error;
use std::process::Command;
use std::fmt::{self, Display};

use scenarios::Scenario;
use super::printer::Printer;
use super::children::PreparedChild;


/// The name of the environment variable to hold the scenario name.
const SCENARIOS_NAME_NAME: &'static str = "SCENARIOS_NAME";


/// A convenience type for shortening the results in this module.
type Result<T> = ::std::result::Result<T, VariableNameError>;


/// A wrapper around the customization flags of a `CommandLine`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Start child processes in a clean environment.
    ///
    /// If `true`, child processes only receive those environment
    /// variables that are defined in a scenario.
    /// If `false`, child processes inherit the environment of this
    /// process, updated with the variables of their respective
    /// scenario.
    ///
    /// The default is `false`.
    pub ignore_env: bool,
    /// Replace "{}" with the scenario name in the command line.
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
    /// whose name is defined in `SCENARIOS_NAME_NAME`. This variable
    /// contains the name of the scenario in which the child process is
    /// being executed.
    ///
    /// The default is `true`.
    pub add_scenarios_name: bool,
    /// Check for previous definitions of "SCENARIOS_NAME".
    ///
    /// If `true`, it is an error to set `add_scenarios_name` to `true`
    /// *and* supply your own environment variable whose name is equal
    /// to `SCENARIOS_NAME_NAME`.
    /// If this is `false` and `add_scenarios_name` is `true`, such a
    /// variable gets silently overwritten.
    /// If `add_scenarios_name` is `false`, this has option no effect.
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


/// A `Consumer` of `Scenario`s that executes a command line in them.
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
pub struct CommandLine<S: AsRef<str>> {
    /// The command line containing the program and its arguments.
    command_line: Vec<S>,
    /// Flags to customize the creation of child processes.
    options: Options,
}

impl<S: AsRef<str>> CommandLine<S> {
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

    /// Like `new()`, but allows you to also specify the options.
    pub fn with_options<I>(command_line: I, options: Options) -> Option<Self>
    where
        I: IntoIterator<Item = S>,
    {
        let command_line = command_line.into_iter().collect::<Vec<_>>();
        if command_line.is_empty() {
            None
        } else {
            CommandLine {
                    command_line,
                    options,
                }
                .into()
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
    /// a variable definition for `SCENARIOS_NAME` even though this
    /// command line is instructed to add such a variable itself. (See
    /// documentation of `Options` for more information.)
    pub fn with_scenario(&self, scenario: Scenario) -> Result<PreparedChild> {
        let (name, variables) = scenario.into_parts();
        let command = self.create_command(variables, &name)?;
        Ok(PreparedChild::new(name.into_owned(), command))
    }

    /// Like `with_scenario`, but does not consume the `Scenario`.
    pub fn with_scenario_ref(&self, scenario: &Scenario) -> Result<PreparedChild> {
        self.create_command(scenario.variables(), scenario.name())
            .map(|command| PreparedChild::new(scenario.name().to_owned(), command),)
    }

    /// Internal implementation of `with_scenario`.
    fn create_command<I, K, V, N>(&self, env_vars: I, name: N) -> Result<Command>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
        N: AsRef<str>,
    {
        let mut cmd = Command::new(self.program().as_ref());
        // Go through each of the options and prepare `cmd` accordingly.
        if self.options.insert_name_in_args {
            self.add_args_formatted(&mut cmd, name.as_ref());
        } else {
            cmd.args(self.args().iter().map(AsRef::as_ref));
        }
        if self.options.ignore_env {
            cmd.env_clear();
        }
        if self.options.add_scenarios_name && self.options.is_strict {
            Self::add_vars_checked(&mut cmd, env_vars)?;
        } else {
            cmd.envs(env_vars);
        }
        if self.options.add_scenarios_name {
            cmd.env(SCENARIOS_NAME_NAME, name.as_ref());
        }
        Ok(cmd)
    }

    /// Inserts `name` into `self.args()` before adding them to `cmd`.
    fn add_args_formatted<N: AsRef<str>>(&self, cmd: &mut Command, name: N) {
        // We treat each argument as a template in which `name` is
        // inserted before being added to `cmd`.
        let mut printer = Printer::new_null();
        for arg in self.args().iter() {
            printer.set_template(arg.as_ref());
            cmd.arg(printer.format(name.as_ref()));
        }
    }

    /// Checks the name of each variable before adding it to `cmd`.
    fn add_vars_checked<I, K, V>(cmd: &mut Command, env_vars: I) -> Result<()>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (k, v) in env_vars.into_iter() {
            if k.as_ref() == SCENARIOS_NAME_NAME {
                return Err(VariableNameError);
            }
            cmd.env(k, v);
        }
        Ok(())
    }
}


#[derive(Debug)]
pub struct VariableNameError;

impl Display for VariableNameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl Error for VariableNameError {
    fn description(&self) -> &str {
        "bad variable name: SCENARIOS_NAME"
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}


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
