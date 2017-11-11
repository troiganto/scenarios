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


#![allow(dead_code)]

#[macro_use]
extern crate clap;
extern crate num_cpus;
#[macro_use]
extern crate quick_error;

mod app;
mod logger;
mod scenarios;
mod cartesian;
mod consumers;


use scenarios::{Scenario, ScenarioFile};
use consumers::{PreparedChild, FinishedChild};


/// The entry point and wrapper around `try_main`.
fn main() {
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
            logger::Logger::new(args.is_present("quiet")).log(err);
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
fn try_main(args: &clap::ArgMatches) -> Result<(), Error> {
    // Collect scenario file names into a vector of vectors of scenarios.
    // Each inner vector represents one input file.
    let is_strict = !args.is_present("lax");
    let scenario_files: Vec<ScenarioFile> = args.values_of("input")
        .ok_or(Error::NoScenarios)?
        .map(|path| ScenarioFile::from_file_or_stdin(path, is_strict))
        .collect::<Result<_, _>>()?;
    let all_scenarios: Vec<Vec<Scenario>> = scenario_files
        .iter()
        .map(|f| f.iter().collect::<Result<_, _>>())
        .collect::<Result<_, _>>()?;

    // Go through all possible combinations of scenarios and a merged
    // scenario for each of them. Hand these merged scenarios then over
    // to the correct handler.
    let merge_options = scenarios::MergeOptions {
        delimiter: args.value_of("delimiter").expect("default value"),
        is_strict: is_strict,
    };
    let combined_scenarios =
        cartesian::product(&all_scenarios).map(|set| Scenario::merge_all(set, merge_options));
    if args.is_present("command_line") {
        let handler = CommandLineHandler::new(&args);
        consumers::loop_in_process_pool(combined_scenarios, handler)?;
    } else {
        handle_printing(&args, combined_scenarios)?;
    }
    Ok(())
}


/// Prints the given scenarios to stdout.
///
/// # Errors
/// This fails if two variable names conflict and strict mode is
/// enabled.
fn handle_printing<'s, I>(args: &clap::ArgMatches, scenarios: I) -> Result<(), Error>
where
    I: Iterator<Item = scenarios::Result<Scenario<'s>>>,
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
    /// Argument read from --jobs.
    max_num_of_children: usize,
    /// The command line that is executed for each scenario.
    command_line: consumers::CommandLine<&'a str>,
    /// A logger that helps us print information to the user.
    logger: logger::Logger<'static>,
    ///
    any_errors: bool,
}

impl<'a> CommandLineHandler<'a> {
    /// Creates a new handler.
    ///
    /// This reads the parsed command-line arguments and initializes
    /// the fields of this struct from them.
    pub fn new(args: &'a clap::ArgMatches) -> Self {
        CommandLineHandler {
            keep_going: args.is_present("keep_going"),
            max_num_of_children: Self::max_num_tokens_from_args(args),
            command_line: Self::command_line_from_args(args),
            logger: logger::Logger::new(args.is_present("quiet")),
            any_errors: false,
        }
    }

    /// Creates a `CommandLine` from `args`.
    fn command_line_from_args(args: &'a clap::ArgMatches) -> consumers::CommandLine<&'a str> {
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
        args.values_of("command_line")
            .and_then(|argv| consumers::CommandLine::with_options(argv, options))
            .unwrap()
    }

    /// Parses and interprets the `--jobs` option.
    fn max_num_tokens_from_args(args: &clap::ArgMatches) -> usize {
        if !args.is_present("jobs") {
            return 1;
        }
        // We can unwrap the `parse()` result because clap validates --jobs.
        args.value_of("jobs")
            .map(|s| s.parse().unwrap())
            .unwrap_or_else(num_cpus::get)
    }
}

impl<'a, 's> consumers::LoopDriver<scenarios::Result<Scenario<'s>>> for CommandLineHandler<'a> {
    type Error = Error;

    fn max_num_of_children(&self) -> usize {
        self.max_num_of_children
    }

    fn prepare_child(&self, s: scenarios::Result<Scenario<'s>>) -> Result<PreparedChild, Error> {
        let child = self.command_line.with_scenario(s?)?;
        Ok(child)
    }

    fn on_reap(&mut self, child: FinishedChild) -> Result<(), Self::Error> {
        let result = child.into_result();
        if self.keep_going {
            if let Err(err) = result {
                // Don't convert error to `Self::Error` -- that would add the
                // word "error:" to the log string. But we don't want that
                // because we keep running.
                self.any_errors = true;
                self.logger.log(err)
            }
            Ok(())
        } else {
            result.map_err(Error::from)
        }
    }

    fn on_loop_failed(&mut self, error: Self::Error) {
        self.any_errors = true;
        self.logger.log(error);
        if self.max_num_of_children > 1 {
            self.logger.log("waiting for unfinished jobs ...");
        }
    }

    fn on_cleanup_reap(&mut self, child: Result<FinishedChild, consumers::ChildError>) {
        if let Err(err) = child.and_then(FinishedChild::into_result) {
            // Don't convert error to `Self::Error` -- that would add the word
            // "error:" to the log string. But we don't want that because we
            // keep running.
            self.logger.log(err);
        }
    }

    fn on_finish(self) -> Result<(), Self::Error> {
        if !self.any_errors {
            Ok(())
        } else {
            Err(Error::NotAllFinished)
        }
    }
}


quick_error! {
    #[derive(Debug)]
    enum Error {
        ParseError(err: scenarios::ParseError) {
            description(err.description())
            display("error: {}", err)
            cause(err)
            from()
        }
        ScenarioError(err: scenarios::ScenarioError) {
            description(err.description())
            display("error: {}", err)
            cause(err)
            from()
        }
        VariableNameError(err: consumers::VariableNameError) {
            description(err.description())
            display("error: {}", err)
            cause(err)
            from()
        }
        ChildError(err: consumers::ChildError) {
            description(err.description())
            display("error: {}", err)
            cause(err)
            from()
        }
        NotAllFinished {
            description("not all scenarios terminated successfully")
        }
        NoScenarios {
            description("error: no scenarios provided")
        }
    }
}
