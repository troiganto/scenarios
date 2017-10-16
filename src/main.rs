#![allow(dead_code)]

#[macro_use]
extern crate clap;
extern crate regex;
extern crate num_cpus;
#[macro_use]
extern crate lazy_static;

mod app;
mod scenarios;
mod cartesian;
mod consumers;
mod intoresult;


use std::io;
use std::time;
use std::thread;
use std::num::ParseIntError;
use std::fmt::{self, Display};
use std::error::Error as StdError;

use scenarios::Scenario;
use consumers::commandline::{self, CommandLine};
use intoresult::{CommandFailed, IntoResult};


fn main() {
    // Get clapp::App instance.
    let app = app::get_app();
    // We clone `app` here because `get_matches` consumes it -- but we
    // might still need it when handling -h and --help.
    let args = app.clone().get_matches();
    // Handle -h (short help) and --help (long help).
    if args.is_present("short_help") {
        app::print_short_help(app);
        return;
    } else if args.is_present("long_help") {
        app::print_long_help(app);
        return;
    }
    // Delegate to `try_main`. Catch any error, print it to stderr, and
    // exit with code 1.
    if let Err(err) = try_main(&args) {
        let msg = err.to_string();
        let kind = clap::ErrorKind::Format;
        let err = clap::Error::with_description(&msg, kind);
        err.exit();
    }
}


fn try_main(args: &clap::ArgMatches) -> Result<(), Error> {
    // Collect scenario file names.
    let scenario_files: Vec<Vec<Scenario>> = args.values_of("input")
        .ok_or(Error::NoScenarios)?
        .map(scenarios::from_file_or_stdin)
        .collect::<Result<_, _>>()?;

    // Create and configure a scenarios merger.
    let merger = scenarios::Merger::new()
        .with_delimiter(
            args.value_of("delimiter")
                .expect("default value is missing"),
        )
        .with_strict_mode(!args.is_present("lax"));

    // Use the merger to get a list of all combinations of scenarios.
    let combined_scenarios = cartesian::product(&scenario_files).map(
        |set_of_scenarios| {
            merger.merge(set_of_scenarios.into_iter())
        },
    );

    if args.is_present("command_line") {
        handle_command_line(combined_scenarios, &args)
    } else {
        handle_printing(combined_scenarios, &args)
    }
}


fn handle_command_line<I>(scenarios: I, args: &clap::ArgMatches) -> Result<(), Error>
where
    I: Iterator<Item = Result<Scenario, scenarios::MergeError>>,
{
    // Read the arguments.
    let keep_going = args.is_present("keep_going");
    let command_line = command_line_from_args(args)?;
    let mut token_stock = if let Some(num) = args.value_of("jobs") {
        consumers::TokenStock::new(num.parse::<usize>()?)
    } else if args.is_present("jobs") {
        consumers::TokenStock::new(num_cpus::get())
    } else {
        consumers::TokenStock::new(1)
    };
    let mut children = consumers::ProcessPool::with_capacity(token_stock.num_remaining());
    // Iterate over all scenarios. Because `children` panicks if we
    // drop it while it's still full, we use an anonymous function to
    // let no result escape. TODO: Wait for `catch_expr`.
    let run_result: Result<(), Error> = (|| {
        for scenario in scenarios {
            let scenario = scenario?;
            let mut waiting_for_token = true;
            while waiting_for_token {
                // Clear out finished children and check for errors.
                for (exit_status, token) in children.reap() {
                    token_stock.return_token(token);
                    if !keep_going {
                        exit_status.into_result()?;
                    }
                }
                // If there are free tokens, we take one and start a new process.
                // Otherwise, we just wait and try again.
                if let Some(token) = token_stock.get_token() {
                    waiting_for_token = false;
                    let mut command = command_line.with_scenario(&scenario)?;
                    children.push(command.spawn()?, token);
                } else {
                    thread::sleep(time::Duration::from_millis(10))
                }
            }
        }
        Ok(())
    })();
    let exit_statuses = children.join_all();
    // Here, the pool is empty and we can evaluate all possible errors
    // (if we want to).
    if !keep_going {
        run_result?;
        for (exit_status, _) in exit_statuses {
            exit_status.into_result()?;
        }
    }
    Ok(())
}


fn handle_printing<I>(scenarios: I, args: &clap::ArgMatches) -> Result<(), Error>
where
    I: Iterator<Item = Result<Scenario, scenarios::MergeError>>,
{
    let mut printer = consumers::Printer::new();
    if args.is_present("print0") {
        printer.set_terminator("\0");
    }
    if let Some(template) = args.value_of("print0").or(args.value_of("print")) {
        printer.set_template(template);
    }
    for scenario in scenarios {
        printer.print_scenario(&scenario?);
    }
    Ok(())
}


fn command_line_from_args<'a>(args: &'a clap::ArgMatches) -> Result<CommandLine<'a>, Error> {
    // Configure the command line.
    let command_line: Vec<_> = args.values_of("command_line")
        .ok_or(Error::NoCommandLine)?
        .collect();
    let mut command_line = consumers::CommandLine::new(command_line)
        .ok_or(Error::NoCommandLine)?;
    command_line.ignore_env = args.is_present("ignore_env");
    command_line.insert_name_in_args = !args.is_present("no_insert_name");
    command_line.add_scenarios_name = !args.is_present("no_export_name");
    command_line.is_strict = !args.is_present("lax");
    Ok(command_line)
}


#[derive(Debug)]
enum Error {
    IoError(io::Error),
    FileParseError(scenarios::FileParseError),
    ScenarioError(scenarios::ScenarioError),
    VariableNameError(commandline::VariableNameError),
    CommandFailed(CommandFailed),
    ParseIntError(ParseIntError),
    NoScenarios,
    NoCommandLine,
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IoError(ref err) => err.fmt(f),
            Error::FileParseError(ref err) => err.fmt(f),
            Error::ScenarioError(ref err) => err.fmt(f),
            Error::ParseIntError(ref err) => err.fmt(f),
            Error::VariableNameError(ref err) => err.fmt(f),
            Error::CommandFailed(ref err) => err.fmt(f),
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::IoError(ref err) => err.description(),
            Error::FileParseError(ref err) => err.description(),
            Error::ScenarioError(ref err) => err.description(),
            Error::VariableNameError(ref err) => err.description(),
            Error::CommandFailed(ref err) => err.description(),
            Error::ParseIntError(ref err) => err.description(),
            Error::NoScenarios => "no scenarios provided",
            Error::NoCommandLine => "no command line provided",
        }
    }

    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            Error::IoError(ref err) => Some(err),
            Error::FileParseError(ref err) => Some(err),
            Error::ScenarioError(ref err) => Some(err),
            Error::VariableNameError(ref err) => Some(err),
            Error::CommandFailed(ref err) => Some(err),
            Error::ParseIntError(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::IoError(err)
    }
}

impl From<scenarios::FileParseError> for Error {
    fn from(err: scenarios::FileParseError) -> Self {
        Error::FileParseError(err)
    }
}

impl From<scenarios::ScenarioError> for Error {
    fn from(err: scenarios::ScenarioError) -> Self {
        Error::ScenarioError(err)
    }
}

impl From<scenarios::MergeError> for Error {
    fn from(err: scenarios::MergeError) -> Self {
        match err {
            scenarios::MergeError::NoScenarios => Error::NoScenarios,
            scenarios::MergeError::ScenarioError(err) => Error::ScenarioError(err),
        }
    }
}

impl From<commandline::VariableNameError> for Error {
    fn from(err: commandline::VariableNameError) -> Self {
        Error::VariableNameError(err)
    }
}

impl From<CommandFailed> for Error {
    fn from(err: CommandFailed) -> Self {
        Error::CommandFailed(err)
    }
}

impl From<ParseIntError> for Error {
    fn from(err: ParseIntError) -> Self {
        Error::ParseIntError(err)
    }
}
