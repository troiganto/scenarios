
use std::ffi::OsStr;
use std::error::Error;
use std::process::Command;
use std::fmt::{self, Display};

use scenarios::Scenario;
use super::Printer;


/// The name of the environment variable to hold the scenario name.
const SCENARIOS_NAME_NAME: &'static str = "SCENARIOS_NAME";


/// Convenience alias for the `Result` type.
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
/// The scenario's variable definitions are set as environment
/// variables of the command line. The scenario's name can either be
/// inserted into the command line itself or set as an additional
/// environment variable.
///
/// `CommandLine` is generic over the backing buffer that contains the
/// command line. The only condition is that it can be cast via `AsRef`
/// to a slice of string slices (`&[&str]`). By default, a `Vec` is
/// used.
pub struct CommandLine<'a, Buffer = Vec<&'a str>>
where
    Buffer: AsRef<[&'a str]>,
{
    /// The command line containing the program and its arguments.
    command_line: Buffer,
    /// Flags to customize the creation of child processes.
    options: Options,
    /// Phantom data to connect this object's lifetime to that of the
    /// string slices in the backing buffer.
    _lifetime: ::std::marker::PhantomData<&'a ()>,
}

// FIXME: Improve this interface.
impl<'a, Buffer> CommandLine<'a, Buffer>
where
    Buffer: AsRef<[&'a str]>,
{
    /// Creates a new instance wrapping a command line.
    ///
    /// The backing buffer should contain the program to be executed
    /// as well as all its arguments. The result is `None` if the
    /// backing buffer is empty, otherwise it is `Some(CommandLine)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let line = vec!["echo", "-n", "Hello World!"];
    /// let expected = &line;
    /// let cl = CommandLine::new(line.clone()).unwrap();
    /// assert_eq!(cl.command_line(), &line);
    ///
    /// /// The backing buffer must not be empty.
    /// assert!(CommandLine::new(Vec::new()).is_none());
    /// ```
    pub fn new(command_line: Buffer) -> Option<Self> {
        Self::with_options(command_line, Default::default())
    }

    /// Like `new()`, but allows you to also specify the options.
    pub fn with_options(command_line: Buffer, options: Options) -> Option<Self> {
        if !command_line.as_ref().is_empty() {
            Some(
                CommandLine {
                    command_line,
                    options,
                    _lifetime: Default::default(),
                },
            )
        } else {
            None
        }
    }

    pub fn command_line(&self) -> &[&'a str] {
        self.command_line.as_ref()
    }

    pub fn options(&self) -> &Options {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut Options {
        &mut self.options
    }

    pub fn set_options(&mut self, options: Options) {
        self.options = options;
    }

    /// Returns the program and its arguments
    ///
    /// #Panics
    /// This panics if, for whatever reason, the backing buffer is
    /// empty. The checks in `CommandLine::new()` should prevent that.
    pub fn program_args(&self) -> (&'a str, &[&'a str]) {
        let (&program, args) = self.command_line()
            .split_first()
            .expect("command line is empty");
        (program, args)
    }

    /// Prepare an `std::process::Command` from this command line.
    ///
    /// The returned `Command` can be used to spawn a child process.
    pub fn with_scenario(&self, scenario: &Scenario) -> Result<Command> {
        self.create_command(scenario.variables(), scenario.name())
    }

    /// Creates an `std::process::Command` corresponding to this line.
    ///
    /// The parameter `env_vars` should be set to the environment
    /// variables to add before executing the command. The parameter
    /// `name` is the name of the scenario to execute.
    pub fn create_command<I, K, V, N>(&self, env_vars: I, name: N) -> Result<Command>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
        N: AsRef<str> + AsRef<OsStr>,
    {
        let (program, args) = self.program_args();
        let mut cmd = Command::new(program);
        // If we want to insert the name into the args, we have to
        // iterate over the args -- otherwise, we can pass them as a
        // whole.
        if self.options.insert_name_in_args {
            let mut printer = Printer::new("", "");
            for arg in args {
                printer.set_template(arg);
                cmd.arg(printer.format(name.as_ref()));
            }
        } else {
            cmd.args(args);
        }
        // Set environment variables.
        if self.options.ignore_env {
            cmd.env_clear();
        }
        for (k, v) in env_vars.into_iter() {
            if self.options.add_scenarios_name && self.options.is_strict &&
               k.as_ref() == SCENARIOS_NAME_NAME {
                return Err(VariableNameError);
            }
            cmd.env(k, v);
        }
        if self.options.add_scenarios_name {
            cmd.env(SCENARIOS_NAME_NAME, name);
        }
        Ok(cmd)
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
    use super::*;

    #[test]
    fn test_echo() {
        let cl = CommandLine::new(["echo", "-n"]).unwrap();
        let env: &[(&str, &str)] = &[];
        cl.create_command(env.into_iter().cloned(), "")
            .expect("CommandLine::create_command failed")
            .status()
            .expect("Child::status failed");
    }

    #[test]
    fn test_insert_name() {
        let mut cl = CommandLine::new(["echo", "a cool {}!"]).unwrap();
        cl.insert_name_in_args = true;
        let env: &[(&str, &str)] = &[];
        let output = cl.create_command(env.into_iter().cloned(), "name")
            .expect("CommandLine::create_command failed")
            .output()
            .expect("Child::output failed");
        let output = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output, "a cool name!\n".to_owned());
    }
}
