#![allow(dead_code)]

#[macro_use]
extern crate clap;
extern crate num_cpus;
#[macro_use]
extern crate quick_error;

mod app;
mod scenarios;
mod cartesian;
mod consumers;


use std::io;
use std::num::ParseIntError;

use scenarios::Scenario;
use consumers::commandline::{self, CommandLine};
use consumers::children;
use consumers::pool;


/// The entry point and wrapper around `try_main`.
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


/// The actual main function.
///
/// It receives the fully parsed arguments and may return an error.
fn try_main(args: &clap::ArgMatches) -> Result<(), Error> {
    // Collect scenario file names into a vector of vectors of scenarios.
    // Each inner vector represents one input file.
    let scenario_files = args.values_of("input")
        .ok_or(Error::NoScenarios)?
        .map(scenarios::from_file_or_stdin)
        .collect::<Result<Vec<Vec<Scenario>>, _>>()?;

    // Create and configure a scenarios merger.
    let merger = scenarios::Merger::new()
        .with_delimiter(
            args.value_of("delimiter")
                .expect("default value is missing"),
        )
        .with_strict_mode(!args.is_present("lax"));

    // Use the merger to get a list of all combinations of scenarios.
    // Hand these then over to the correct handler.
    let combined_scenarios = cartesian::product(&scenario_files).map(|set| merger.merge(set));
    if args.is_present("command_line") {
        CommandLineHandler::new(&args)?
            .handle(combined_scenarios)
    } else {
        handle_printing(&args, combined_scenarios)
    }
}


/// Prints the given scenarios to stdout.
///
/// # Errors
/// This fails if two variable names conflict and strict mode is
/// enabled.
fn handle_printing<I>(args: &clap::ArgMatches, scenarios: I) -> Result<(), Error>
where
    I: Iterator<Item = Result<Scenario, scenarios::MergeError>>,
{
    let mut printer = consumers::Printer::default();
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


/// Helper struct that breaks up the task of executing a command line.
struct CommandLineHandler<'a> {
    /// Flag read from --keep-going.
    keep_going: bool,
    /// The command line that is executed for each scenario.
    command_line: consumers::CommandLine<&'a str>,
    /// Our stock of tokens for parallelism.
    tokens: consumers::TokenStock,
    /// A pool of currently-running processes.
    children: consumers::ProcessPool,
}

impl<'a> CommandLineHandler<'a> {
    /// Creates a new handler.
    ///
    /// This reads the parsed command-line arguments and initializes
    /// the fields of this struct from them.
    ///
    /// # Errors
    /// This fails if either `max_num_tokens_from_args()` or
    /// `command_line_from_args()` fails.
    pub fn new(args: &'a clap::ArgMatches) -> Result<Self, Error> {
        let handler = CommandLineHandler {
            keep_going: args.is_present("keep_going"),
            command_line: Self::command_line_from_args(args)?,
            tokens: consumers::TokenStock::new(Self::max_num_tokens_from_args(args)?),
            children: consumers::ProcessPool::new(),
        };
        Ok(handler)
    }

    /// Creates a `CommandLine` froom `args`.
    ///
    /// # Errors
    /// This fails if no or an empty command line was given.
    fn command_line_from_args(args: &'a clap::ArgMatches) -> Result<CommandLine<&'a str>, Error> {
        let options = commandline::Options {
            is_strict: !args.is_present("lax"),
            ignore_env: args.is_present("ignore_env"),
            add_scenarios_name: !args.is_present("no_export_name"),
            insert_name_in_args: !args.is_present("no_insert_name"),
        };
        args.values_of("command_line")
            .and_then(|argv| consumers::CommandLine::with_options(argv, options))
            .ok_or(Error::NoCommandLine)
    }

    /// Parses and interprets the `--jobs` option.
    ///
    /// # Errors
    /// This fails when `--jobs` is given a value that cannot be parsed
    /// as an unsigned integer.
    fn max_num_tokens_from_args(args: &clap::ArgMatches) -> Result<usize, Error> {
        if let Some(num) = args.value_of("jobs") {
            num.parse::<usize>().map_err(Error::from)
        } else if args.is_present("jobs") {
            Ok(num_cpus::get())
        } else {
            Ok(1)
        }
    }

    /// Runs the main loop of the program and tears it down afterwards.
    ///
    /// This immediately calls `inner_loop()` and afterwards waits for
    /// all running child processes.
    ///
    /// # Errors
    /// Same as `inner_loop()`.
    pub fn handle<I>(&mut self, scenarios: I) -> Result<(), Error>
    where
        I: Iterator<Item = Result<Scenario, scenarios::MergeError>>,
    {
        let run_result = self.inner_loop(scenarios);
        // Reap all remaining children.
        let finished_children = self.children.wait_and_reap_all();
        // Here, the pool is empty and we can evaluate all possible errors.
        // I/O errors always get evaluated, exit statuses only maybe.
        run_result?;
        for (child, _token) in finished_children {
            let child = child?;
            if !self.keep_going {
                child.into_result()?;
            }
        }
        Ok(())
    }

    /// The main loop of the program.
    ///
    /// This starts one child process for each scenario and pushes it
    /// into the pool `children`. If the maximum allowed number of
    /// child processes is running, this loop waits until one of them
    /// has finished.
    ///
    /// # Errors
    /// This function may fail if:
    /// - spawning a child process gives an `io::Error`;
    /// - waiting on a child process gives an `io::Error`;
    /// - two variable names conflict and strict mode is enabled;
    /// - a child process exits with non-zero exit status and
    ///   `keep_going` is `false`.
    fn inner_loop<I>(&mut self, scenarios: I) -> Result<(), Error>
    where
        I: Iterator<Item = Result<Scenario, scenarios::MergeError>>,
    {
        // Copy `keep_going` to avoid borrowing `self` to the closure.
        let keep_going = self.keep_going;
        let reaper = |child: children::FinishedChild| if keep_going {
            Ok(())
        } else {
            child.into_result()
        };
        for scenario in scenarios {
            let token = pool::spin_wait_for_token(&mut self.tokens, &mut self.children, &reaper)?;
            let child = self.command_line
                .with_scenario(scenario?)?
                .spawn_or_return_token(token, &mut self.tokens)?;
            self.children.push(child);
        }
        Ok(())
    }
}


quick_error! {
    #[derive(Debug)]
    enum Error {
        IoError(err: io::Error) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        ParseError(err: scenarios::ParseError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        ScenarioError(err: scenarios::ScenarioError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        VariableNameError(err: commandline::VariableNameError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        ChildError(err: children::Error) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        ParseIntError(err: ParseIntError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
        NoScenarios {
            description("no scenarios provided")
        }
        NoCommandLine {
            description("no command line provided")
        }
    }
}

impl From<scenarios::MergeError> for Error {
    fn from(err: scenarios::MergeError) -> Self {
        match err {
            scenarios::MergeError::NoScenarios => Error::NoScenarios,
            scenarios::MergeError::ScenarioError(err) => Error::from(err),
        }
    }
}
