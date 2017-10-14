#![allow(dead_code)]

#[macro_use]
extern crate clap;
extern crate regex;
extern crate num_cpus;
#[macro_use]
extern crate lazy_static;

mod scenarios;
mod cartesian;
mod consumers;
mod intoresult;


use std::io;
use std::num::ParseIntError;
use std::fmt::{self, Display};
use std::error::Error as StdError;

use clap::{Arg, ArgGroup, App, AppSettings};

use scenarios::Scenario;
use consumers::CommandLine;
use intoresult::{CommandFailed, IntoResult};


fn main() {
    let app = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .setting(clap::AppSettings::TrailingVarArg)
        .setting(AppSettings::DeriveDisplayOrder)
        // General args.
        // We create our own --help so that the arguments are correctly
        // ordered.
        .arg(Arg::with_name("long_help")
             .long("help")
             .help("Print detailed help information."))
        .arg(Arg::with_name("short_help")
             .short("h")
             .help("Print short help information."))
        .arg(Arg::with_name("delimiter")
             .short("d")
             .long("delimiter")
             .takes_value(true)
             .value_name("STRING")
             .default_value(", ")
             // We print the default ourselves because the by default
             // (hahah), no quotes are printed around the value.
             .hide_default_value(true)
             .help("A delimiter to use when merging the names of a \
                    scenario combination. [default: ', ']"))
        // Strict mode control.
        .group(ArgGroup::with_name("strict_mode")
               .args(&["strict", "lax"])
               .required(false))
        .arg(Arg::with_name("strict")
             .short("s")
             .long("strict")
             .help("This is the default. No two scenario files may \
                    define the same scenario name or environment \
                    variable."))
        .arg(Arg::with_name("lax")
             .short("l")
             .long("lax")
             .help("Disable strict mode."))
        // Input control.
        .arg(Arg::with_name("input")
             .takes_value(true)
             .value_name("SCENARIO FILES")
             .multiple(true)
             .help("Input scenario files. If multiple files are \
                    passed, all possible combinations between them \
                    are used. Pass '-' to read from stdin."))
        // Only one of --print, --print0, and <command> may be passed.
        .group(ArgGroup::with_name("output")
            .args(&["print", "print0", "command_line"])
            .required(false))
        // Scenario name printing.
        .arg(Arg::with_name("print")
             .long("print")
             .takes_value(true)
             .value_name("FORMAT")
             .min_values(0)
             .max_values(1)
             .help("Do not execute a command, just print \
                    SCENARIOS_NAME for all combinations of scenarios \
                    to stdout. Names are separated by newlines. An \
                    optional format string may be passed, in which \
                    '{}' gets replaced with SCENARIOS_NAME."))
        .arg(Arg::with_name("print0")
             .long("print0")
             .takes_value(true)
             .value_name("FORMAT")
             .min_values(0)
             .max_values(1)
             .help("Like --print, but separate scenario combinations \
                    with a null byte instead of a newline. (This is \
                    useful in combination with 'xargs -0'.)"))
        // Command line execution.
        .arg(Arg::with_name("command_line")
             .takes_value(true)
             .value_name("COMMAND")
             .multiple(true)
             .last(true)
             .help("The command line to execute."))
        .arg(Arg::with_name("ignore_env")
             .short("I")
             .long("ignore-env")
             .requires("command_line")
             .help("Do not export the current environment the \
                    subshells."))
        .arg(Arg::with_name("no_insert_name")
             .long("no-insert-name")
             .requires("command_line")
             .help("Do not replace '{}' with SCENARIOS_NAME in the \
                    command line."))
        .arg(Arg::with_name("no_name_variable")
             .long("no-name-variable")
             .requires("command_line")
             .help("Do not export the environment variable \
                    SCENARIOS_NAME to the subshells."))
        // Multi-processing.
        .arg(Arg::with_name("jobs")
             .short("j")
             .long("jobs")
             .requires("command_line")
             .takes_value(true)
             .value_name("N")
             .min_values(0)
             .max_values(1)
             .validator(|s| if s.parse::<usize>().is_ok() { Ok(()) } else { Err(s) })
            .help("The number of scenarios to execute in parallel. \
                   If no number is passed, the number of CPUs on \
                   this machine is used."));

    // We clone `app` here because `get_matches` consumes it -- but we
    // might still need it to print the short help!
    let args = app.clone().get_matches();

    // If -h was passed, reduce the help message to nothing and print
    // it.
    if args.is_present("short_help") {
        app.after_help("").print_help().unwrap();
        return;
    } else if args.is_present("long_help") {
        app.after_help(LONG_EXPLANATION).print_help().unwrap();
        return;
    }

    // Catch all errors, print them to stderr, and exit with code 1.
    if let Err(err) = try_main(&args) {
        let msg = err.to_string();
        let kind = clap::ErrorKind::Format;
        let err = clap::Error::with_description(&msg, kind);
        err.exit();
    }
}


fn try_main<'a>(args: &clap::ArgMatches<'a>) -> Result<(), Error> {
    // Collect scenario file names.
    let scenario_files: Vec<Vec<Scenario>> = args.values_of("input")
        .ok_or(Error::NoScenarios)?
        .map(scenarios::from_file)
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


fn handle_command_line<'a, I>(scenarios: I, args: &clap::ArgMatches<'a>) -> Result<(), Error>
where
    I: Iterator<Item = Result<Scenario, scenarios::MergeError>>,
{
    // Read the arguments.
    let keep_going = args.is_present("keep_going");
    let command_line = command_line_from_args(args)?;
    let mut pool = if let Some(num) = args.value_of("jobs") {
        let num: usize = num.parse()?;
        consumers::ProcessPool::new(num)
    } else if args.is_present("jobs") {
        consumers::ProcessPool::new_automatic()
    } else {
        consumers::ProcessPool::new_trivial()
    };
    // Iterate over all scenarios. Because `pool` panicks if we drop it
    // while it's still full, we use an anonymous function to let no
    // result escape. TODO: Wait for `catch_expr`.
    let run_result = (|| {
        for scenario in scenarios {
            // If the pool is full, we free space and check for errors.
            if let Some(exit_status) = pool.pop_finished_if_full()? {
                if !keep_going {
                    exit_status.into_result()?;
                }
            }
            // Now the pool definitely has space and we can push the
            // new command.
            let command = command_line.with_scenario(&scenario?);
            use consumers::pool::TryPushResult;
            match pool.try_push(command) {
                TryPushResult::CommandSpawned(result) => result?,
                TryPushResult::PoolFull(_) => panic!("pool full despite emptying"),
            };
        }
        Ok(())
    })();
    let exit_statuses = run_result.and(pool.join().map_err(Error::from))?;
    // Here, the pool is empty. If we don't ignore failed child
    // processes, we check each of them and fail on the first error.
    if !keep_going {
        exit_statuses
            .into_iter()
            .map(IntoResult::into_result)
            .collect::<Result<Vec<()>, _>>()?;
    }
    Ok(())
}


fn handle_printing<'a, I>(scenarios: I, args: &clap::ArgMatches<'a>) -> Result<(), Error>
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


fn command_line_from_args<'a>(args: &'a clap::ArgMatches<'a>) -> Result<CommandLine<'a>, Error> {
    // Configure the command line.
    let command_line: Vec<_> = args.values_of("command_line")
        .ok_or(Error::NoCommandLine)?
        .collect();
    let mut command_line = consumers::CommandLine::new(command_line)
        .ok_or(Error::NoCommandLine)?;
    command_line.ignore_env = args.is_present("ignore_env");
    command_line.insert_name_in_args = !args.is_present("no_insert_name");
    command_line.add_scenarios_name = !args.is_present("no_name_variable");
    Ok(command_line)
}


#[derive(Debug)]
enum Error {
    IoError(io::Error),
    FileParseError(scenarios::FileParseError),
    ScenarioError(scenarios::ScenarioError),
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


#[cfg_attr(rustfmt, rustfmt_skip)]
static LONG_EXPLANATION: &'static str = "\
This program takes one or more scenario files. A scenario is named \
set of environment variables to apply at the same time. A scenario \
file contains a list of scenarios in the following format:

    [First scenario name]
    FIRST_VARIABLE = value
    SECOND_VARIABLE = value

    [Second scenario name]
    FIRST_VARIABLE = value
    SECOND_VARIABLE = value

If you pass several scenario files, all possible combinations that \
take one scenario from each file are executed. For instance, assume \
you have the following scenario files:

    - `numbers.ini` with scenarios named \"1\", \"2\", and \"3\";
    - `letters.ini` with scenarios named \"a\" and \"b\";

Then, the following call:

    scenarios numbers.ini letters.ini -- some_program

will execute \"some_program\" six times, each time in a different \
environment: \"1, a\"; \"1, b\"; \"2, a\"; \"2, b\"; \"3, a\"; and \
\"3, b\".

If you don't pass a program, the default is to simply print the names \
of all scenario combinations to stdout. The following call:

    scenarios numbers.ini letters.ini

will produce the following output:

    1, a
    1, b
    2, a
    2, b
    3, a
    3, b

It is an error if two files define the same scenario name, or if two \
scenarios from different files define the same environment variable. \
This check can be disabled by passing the --lax option. In that case, \
later definitions of variables will overwrite former definitions.

When running, scenarios adds an additional variable SCENARIOS_NAME to \
each scenario (unless --no-name-variable is passed). This variable \
contains the name of the current combination of scenarios. Strict \
mode will prevent you from defining SCENARIOS_NAME yourself. With the \
--lax option, your own definition will silently be overwritten.
";
