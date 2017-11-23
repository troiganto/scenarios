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


//! `Scenarios` is a command-line tool that allows you to execute the
//! same command multiple times, each time with different environment
//! variables set. When passed multiple lists of environments,
//! `scenarios` goes through all possible combinations between them.
//!
//! `scenarios` is available on [Github][].
//!
//! [Github]: https://github.com/troiganto/scenarios


// This is an application and, as such, contains functionality that is
// not strictly necessary.
#![allow(dead_code)]


#[macro_use]
extern crate clap;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate glob;
extern crate num_cpus;


pub mod app;
pub mod logger;
pub mod trytostr;
pub mod cartesian;
pub mod consumers;
pub mod scenarios;


use std::ffi::OsStr;

use failure::{ResultExt, Error};

use trytostr::OsStrExt;
use consumers::{PreparedChild, FinishedChild};
use scenarios::{MergeError, Scenario, ScenarioFile};


/// The entry point and wrapper around `try_main`.
pub fn main() {
    let exit_code: i32 = {
        // Get clapp::App instance.
        let app = app::get_app();
        // We clone `app` here because `get_matches` consumes it -- but we
        // might still need it when handling -h and --help.
        let args = app.clone().get_matches();
        // Handle -h (short help) and --help (long help).
        if args.is_present("short_help") {
            app::print_short_help(app);
            0
        } else if args.is_present("long_help") {
            app::print_long_help(app);
            0
        }
        // Delegate to `try_main`. Catch any error, print it to stderr, and
        // exit with code 1.
        else if let Err(err) = try_main(&args) {
            // We want `SomeScenariosFailed` to be printed as a regular info,
            // but all other errors with the full chain.
            let logger = logger::Logger::new(args.is_present("quiet"));
            match err.downcast::<SomeScenariosFailed>() {
                Ok(err) => logger.log(err),
                Err(err) => logger.log_error_chain(err),
            }
            1
        } else {
            // `try_main()` returned Ok(()).
            0
        }
    };
    // All destructors have run at this point.
    ::std::process::exit(exit_code);
}


/// The actual main function.
///
/// It receives the fully parsed arguments and may return an error.
pub fn try_main(args: &clap::ArgMatches) -> Result<(), Error> {
    // Collect scenario file names into a vector of vectors of scenarios.
    // Each inner vector represents one input file.
    let is_strict = !args.is_present("lax");
    let delimiter = args.value_of_os("delimiter")
        .expect("default value")
        .try_to_str()
        .context("invalid value for --delimiter")?;
    let scenario_files: Vec<ScenarioFile> = args.values_of_os("input")
        .ok_or(NoScenarios)?
        .map(|path| ScenarioFile::from_cl_arg(path, is_strict))
        .collect::<Result<_, _>>()
        .context("could not read file")?;
    let all_scenarios: Vec<Vec<Scenario>> = scenario_files
        .iter()
        .map(|f| f.iter().collect::<Result<_, _>>())
        .collect::<Result<_, _>>()
        .context("could not build scenarios")?;

    // Go through all possible combinations of scenarios and a merged
    // scenario for each of them. Hand these merged scenarios then over
    // to the correct handler.
    let merge_opts = scenarios::MergeOptions { delimiter, is_strict };
    let combos = cartesian::product(&all_scenarios).map(|set| Scenario::merge_all(set, merge_opts));
    if args.is_present("command_line") {
        let handler = CommandLineHandler::new(&args)?;
        consumers::loop_in_process_pool(combos, handler)?;
    } else {
        handle_printing(&args, combos)?;
    }
    Ok(())
}


/// Prints the given scenarios to stdout.
///
/// # Errors
/// This fails if two variable names conflict and strict mode is
/// enabled.
pub fn handle_printing<'s, I>(args: &clap::ArgMatches, scenarios: I) -> Result<(), Error>
where
    I: Iterator<Item = Result<Scenario<'s>, MergeError>>,
{
    let mut printer = consumers::Printer::default();
    if let Some(template) = args.value_of_os("print0") {
        let template = template
            .try_to_str()
            .context("could not parse --print0 argument")?;
        printer.set_template(template);
        printer.set_terminator("\0");
    } else if let Some(template) = args.value_of_os("print") {
        let template = template
            .try_to_str()
            .context("could not parse --print argument")?;
        printer.set_template(template);
    };
    for scenario in scenarios {
        printer.print_scenario(&scenario?);
    }
    Ok(())
}


/// Helper struct that breaks up the task of executing a command line.
pub struct CommandLineHandler<'a> {
    /// Flag read from --keep-going.
    keep_going: bool,
    /// Argument read from --jobs.
    max_num_of_children: usize,
    /// The command line that is executed for each scenario.
    command_line: consumers::CommandLine<&'a OsStr>,
    /// A logger that helps us print information to the user.
    logger: logger::Logger<'static>,
    /// A flag that is set if any error occurs during processing.
    ///
    /// This is used so we can tell the user something went wrong even
    /// if `keep_going` has been set.
    any_errors: bool,
}

impl<'a> CommandLineHandler<'a> {
    /// Creates a new handler.
    ///
    /// This reads the parsed command-line arguments and initializes
    /// the fields of this struct from them.
    pub fn new(args: &'a clap::ArgMatches) -> Result<Self, Error> {
        let max_num_of_children = Self::max_num_tokens_from_args(args)
            .context("invalid value for --jobs")?;
        let handler = CommandLineHandler {
            any_errors: false,
            max_num_of_children,
            keep_going: args.is_present("keep_going"),
            command_line: Self::command_line_from_args(args),
            logger: logger::Logger::new(args.is_present("quiet")),
        };
        Ok(handler)
    }

    /// Creates a `CommandLine` from `args`.
    fn command_line_from_args(args: &'a clap::ArgMatches) -> consumers::CommandLine<&'a OsStr> {
        let options = consumers::CommandLineOptions {
            is_strict: !args.is_present("lax"),
            ignore_env: args.is_present("ignore_env"),
            add_scenarios_name: !args.is_present("no_export_name"),
            insert_name_in_args: !args.is_present("no_insert_name"),
        };
        // This is only called if the argument `command_line` is
        // present. And since it's a positional argument, i.e. not an
        // --option, being present also means not being empty. Hence,
        // it is safe to unwrap here.
        args.values_of_os("command_line")
            .and_then(|argv| consumers::CommandLine::with_options(argv, options))
            .unwrap()
    }

    /// Parses and interprets the `--jobs` option.
    fn max_num_tokens_from_args(args: &clap::ArgMatches) -> Result<usize, Error> {
        if !args.is_present("jobs") {
            return Ok(1);
        }
        let num = match args.value_of_os("jobs") {
            Some(num) => num,
            None => return Ok(num_cpus::get()),
        };
        let num = num.try_to_str()?;
        let num = num.parse().map_err(|_| NotANumber(num.to_owned()))?;
        Ok(num)
    }
}

impl<'a, 's> consumers::LoopDriver<Result<Scenario<'s>, MergeError>> for CommandLineHandler<'a> {
    fn max_num_of_children(&self) -> usize {
        self.max_num_of_children
    }

    fn prepare_child(&self, s: Result<Scenario<'s>, MergeError>) -> Result<PreparedChild, Error> {
        let child = self.command_line.with_scenario(s?)?;
        Ok(child)
    }

    fn on_reap(&mut self, child: FinishedChild) -> Result<(), Error> {
        let result = child.into_result();
        if self.keep_going {
            if let Err(err) = result {
                // TODO: Avoid logging the word "error" here, because
                // this event does not stop us from running.
                self.any_errors = true;
                self.logger.log_error_chain(err)
            }
            Ok(())
        } else {
            result.map_err(Error::from)
        }
    }

    fn on_loop_failed(&mut self, error: Error) {
        self.any_errors = true;
        self.logger.log_error_chain(error);
        if self.max_num_of_children > 1 {
            self.logger.log("waiting for unfinished jobs ...");
        }
    }

    fn on_cleanup_reap(&mut self, child: Result<FinishedChild, Error>) {
        if let Err(err) = child.and_then(FinishedChild::into_result) {
            // TODO: Avoid logging the word "error" here, because this
            // event does not stop us from running.
            self.logger.log_error_chain(err);
        }
    }

    fn on_finish(self) -> Result<(), Error> {
        if !self.any_errors {
            Ok(())
        } else {
            Err(Error::from(SomeScenariosFailed))
        }
    }
}


#[derive(Debug, Fail)]
#[fail(display = "not all scenarios terminated successfully")]
pub struct SomeScenariosFailed;


#[derive(Debug, Fail)]
#[fail(display = "no scenarios provided")]
pub struct NoScenarios;


#[derive(Debug, Fail)]
#[fail(display = "not a number: {:?}", _0)]
pub struct NotANumber(String);
