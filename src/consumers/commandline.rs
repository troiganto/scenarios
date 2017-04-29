
use std::io;
use std::ffi::OsStr;
use std::process::{Command, ExitStatus};

use scenarios::Scenario;
use super::Consumer;
use super::Printer;


const SCENARIOS_NAME_NAME: &'static str = "SCENARIOS_NAME";


pub struct CommandLine<'a, Buffer>
where
    Buffer: 'a + AsRef<[&'a str]>,
{
    command_line: Buffer,
    inherit_env: bool,
    insert_name_in_args: bool,
    _lifetime: ::std::marker::PhantomData<&'a ()>,
}

impl<'a, Buffer> CommandLine<'a, Buffer>
where
    Buffer: 'a + AsRef<[&'a str]>,
{
    pub fn new(command_line: Buffer) -> Option<Self> {
        if command_line.as_ref().is_empty() {
            return None;
        }
        let result = CommandLine {
            command_line: command_line,
            inherit_env: true,
            insert_name_in_args: false,
            _lifetime: Default::default(),
        };
        Some(result)
    }

    pub fn command_line(&self) -> &[&'a str] {
        self.command_line.as_ref()
    }

    pub fn program_args(&self) -> (&'a str, &[&'a str]) {
        let (&program, args) = self.command_line()
            .split_first()
            .expect("command line is empty");
        (program, args)
    }

    pub fn inherit_env(&self) -> bool {
        self.inherit_env
    }

    pub fn set_inherit_env(&mut self, inherit_env: bool) {
        self.inherit_env = inherit_env;
    }

    pub fn with_inherit_env(mut self, inherit_env: bool) -> Self {
        self.set_inherit_env(inherit_env);
        self
    }

    pub fn insert_name_in_args(&self) -> bool {
        self.insert_name_in_args
    }

    pub fn set_insert_name_in_args(&mut self, insert_name_in_args: bool) {
        self.insert_name_in_args = insert_name_in_args;
    }

    pub fn with_insert_name_in_args(mut self, insert_name_in_args: bool) -> Self {
        self.set_insert_name_in_args(insert_name_in_args);
        self
    }

    pub fn execute<I, K, V, N>(&self, env_vars: I, name: N) -> io::Result<ExitStatus>
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
        if self.insert_name_in_args {
            let mut printer = Printer::new().with_terminator("");
            for arg in args {
                printer.set_template(arg);
                cmd.arg(printer.format(name.as_ref()));
            }
        } else {
            cmd.args(args);
        }
        // Set environment variables.
        if self.inherit_env {
            cmd.env_clear();
        }
        for (k, v) in env_vars.into_iter() {
            cmd.env(k, v);
        }
        cmd.env(SCENARIOS_NAME_NAME, name);
        // Execute.
        cmd.status()
    }
}

impl<'a, Buffer> Consumer for CommandLine<'a, Buffer>
where
    Buffer: 'a + AsRef<[&'a str]>,
{
    fn consume(&self, scenario: &Scenario) {
        self.execute(scenario.variables(), scenario.name())
            .expect("executing process failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_echo() {
        let cl = CommandLine::new(["echo", "-n"])
            .unwrap()
            .with_insert_name_in_args(true);
        let env: &[(&str, &str)] = &[];
        cl.execute(env.into_iter().cloned(), "").unwrap();
    }
}
