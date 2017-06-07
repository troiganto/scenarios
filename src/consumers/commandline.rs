
use std::io;
use std::ffi::OsStr;
use std::process::{Command, Child, ExitStatus, Output};

use scenarios::Scenario;
use super::Printer;


/// The name of the environment variable to hold the scenario name.
const SCENARIOS_NAME_NAME: &'static str = "SCENARIOS_NAME";


/// A `Consumer` of `Scenario`s that executes a command line in them.
///
/// The scenario's variable definitions are set as environment
/// variables of the command line. The scenario's name can either be
/// inserted into the command line itself or set as an additional
/// environment variable.
///
/// `CommandLine` is generic over the backing buffer that contains the
/// command line. The only condition is that it can be cast via `AsRef`
/// to a slice of string slices (`&[&str]`).
pub struct CommandLine<'a, Buffer>
where
    Buffer: 'a + AsRef<[&'a str]>,
{
    /// The command line containing the program and its arguments.
    command_line: Buffer,
    /// If `true`, clear the child process's environment before adding
    /// the scenario's variable definitions.
    pub ignore_env: bool,
    /// If `true`, use a `Printer` to inser the scenario's name into
    /// the command line when executing it.
    pub insert_name_in_args: bool,
    /// If `true`, always define an additional environment variable
    /// with name `SCENARIOS_NAME_NAME` containing the scenario's name.
    pub add_scenarios_name: bool,
    /// Phantom data to connect this object's lifetime to that of the
    /// string slices in the backing buffer.
    _lifetime: ::std::marker::PhantomData<&'a ()>,
}

// FIXME: Improve this interface.
impl<'a, Buffer> CommandLine<'a, Buffer>
where
    Buffer: 'a + AsRef<[&'a str]>,
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
    /// extern crate scenarios;
    /// use scenarios::consumers::CommandLine;
    ///
    /// fn main() {
    ///      let line = vec!["echo", "-n", "Hello World!"];
    ///      let expected = &line;
    ///      let cl = CommandLine::new(line.clone()).unwrap();
    ///      let actual = cl.command_line();
    ///      assert_eq!(expected, actual);
    ///
    ///      /// The backing buffer must not be empty.
    ///      let cl = CommandLine::new(Vec::new());
    ///      assert!(cl.is_none());
    /// }
    /// ```
    pub fn new(command_line: Buffer) -> Option<Self> {
        if command_line.as_ref().is_empty() {
            return None;
        }
        let result = CommandLine {
            command_line: command_line,
            ignore_env: false,
            insert_name_in_args: true,
            add_scenarios_name: true,
            _lifetime: Default::default(),
        };
        Some(result)
    }

    pub fn command_line(&self) -> &[&'a str] {
        self.command_line.as_ref()
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

    /// Spawn a new process from the command line and a given scenario.
    pub fn spawn(&self, scenario: &Scenario) -> io::Result<Child> {
        self.create_command(scenario.variables().into_iter(), scenario.name())
            .spawn()
    }

    /// Executes the command line in the given scenario.
    pub fn execute(&self, scenario: &Scenario) -> io::Result<ExitStatus> {
        self.execute_status(scenario.variables(), scenario.name())
    }

    /// Executes the command line and returns its exit status.
    ///
    /// The parameter `env_vars` should be set to the environment
    /// variables to add before executing the command. The parameter
    /// `name` is the name of the scenario to execute.
    pub fn execute_status<I, K, V, N>(&self, env_vars: I, name: N) -> io::Result<ExitStatus>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
        N: AsRef<str> + AsRef<OsStr>,
    {
        self.create_command(env_vars.into_iter(), name.as_ref())
            .status()
    }

    /// Executes the command line and collect its output.
    ///
    /// The parameter `env_vars` should be set to the environment
    /// variables to add before executing the command. The parameter
    /// `name` is the name of the scenario to execute.
    pub fn execute_output<I, K, V, N>(&self, env_vars: I, name: N) -> io::Result<Output>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
        N: AsRef<str> + AsRef<OsStr>,
    {
        self.create_command(env_vars.into_iter(), name.as_ref())
            .output()
    }

    /// Implementation of `execute_status()` and `execute_output()`.
    fn create_command<I, K, V>(&self, env_vars: I, name: &str) -> Command
    where
        I: Iterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        let (program, args) = self.program_args();
        let mut cmd = Command::new(program);
        // If we want to insert the name into the args, we have to
        // iterate over the args -- otherwise, we can pass them as a
        // whole.
        if self.insert_name_in_args {
            let mut printer = Printer::new().with_terminator("");
            for arg in args {
                printer.set_template(arg);
                cmd.arg(printer.format(name));
            }
        } else {
            cmd.args(args);
        }
        // Set environment variables.
        if self.ignore_env {
            cmd.env_clear();
        }
        for (k, v) in env_vars.into_iter() {
            cmd.env(k, v);
        }
        if self.add_scenarios_name {
            cmd.env(SCENARIOS_NAME_NAME, name);
        }
        cmd
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo() {
        let cl = CommandLine::new(["echo", "-n"]).unwrap();
        let env: &[(&str, &str)] = &[];
        cl.execute_status(env.into_iter().cloned(), "").unwrap();
    }

    #[test]
    fn test_insert_name() {
        let mut cl = CommandLine::new(["echo", "a cool {}!"]).unwrap();
        cl.insert_name_in_args = true;
        let env: &[(&str, &str)] = &[];
        let output = cl.execute_output(env.into_iter().cloned(), "name")
            .unwrap();
        let output = String::from_utf8(output.stdout).unwrap();
        assert_eq!(output, "a cool name!\n".to_owned());
    }
}
