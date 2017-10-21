#![allow(dead_code)]

#[macro_use]
extern crate clap;
extern crate regex;
extern crate num_cpus;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate quick_error;

mod app;
mod scenarios;
mod cartesian;
mod consumers;
mod intoresult;


use std::io;
use std::time;
use std::thread;
use std::num::ParseIntError;

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


fn try_main(args: &clap::ArgMatches) -> Result<(), GlobalError> {
    // Collect scenario file names.
    let scenario_files: Vec<Vec<Scenario>> = args.values_of("input")
        .ok_or(GlobalError::NoScenarios)?
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


fn handle_command_line<I>(scenarios: I, args: &clap::ArgMatches) -> Result<(), GlobalError>
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
    let run_result: Result<(), GlobalError> = (|| {
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


fn handle_printing<I>(scenarios: I, args: &clap::ArgMatches) -> Result<(), GlobalError>
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


fn command_line_from_args<'a>(args: &'a clap::ArgMatches) -> Result<CommandLine<'a>, GlobalError> {
    // Configure the command line.
    let command_line: Vec<_> = args.values_of("command_line")
        .ok_or(GlobalError::NoCommandLine)?
        .collect();
    let mut command_line = consumers::CommandLine::new(command_line)
        .ok_or(GlobalError::NoCommandLine)?;
    command_line.ignore_env = args.is_present("ignore_env");
    command_line.insert_name_in_args = !args.is_present("no_insert_name");
    command_line.add_scenarios_name = !args.is_present("no_export_name");
    command_line.is_strict = !args.is_present("lax");
    Ok(command_line)
}


quick_error! {
    #[derive(Debug)]
    enum GlobalError {
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
        CommandFailed(err: CommandFailed) {
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

impl From<scenarios::MergeError> for GlobalError {
    fn from(err: scenarios::MergeError) -> Self {
        match err {
            scenarios::MergeError::NoScenarios => GlobalError::NoScenarios,
            scenarios::MergeError::ScenarioError(err) => GlobalError::from(err),
        }
    }
}
