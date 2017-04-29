
use std::io;
use std::ffi::OsStr;
use std::process::{Command, ExitStatus};

use scenarios::Scenario;
use super::Consumer;
use super::Printer;


const SCENARIOS_NAME_NAME: &'static str = "SCENARIOS_NAME";


pub struct CommandLine<Buffer>
where
    for<'a> &'a Buffer: IntoIterator<Item = &'a str>,
{
    args: Buffer,
    inherit_env: bool,
    insert_name_in_args: bool,
}

impl<Buffer> CommandLine<Buffer>
where
    for<'a> &'a Buffer: IntoIterator<Item = &'a str>,
{
    pub fn new(args: Buffer) -> Option<Self> {
        if args.into_iter().next().is_none() {
            return None;
        }
        CommandLine {
                args: args,
                inherit_env: true,
                insert_name_in_args: false,
            }
            .into()
    }

    pub fn args(&self) -> <&Buffer as IntoIterator>::IntoIter {
        self.args.into_iter()
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
        let mut args = self.args.into_iter();
        let program = args.next().expect("CommandLine::args is empty");
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

impl<Buffer> Consumer for CommandLine<Buffer>
where
    for<'a> &'a Buffer: IntoIterator<Item = &'a str>,
{
    fn consume(&self, scenario: &Scenario) {
        self.execute(scenario.variables(), scenario.name())
            .expect("executing process failed");
    }
}
