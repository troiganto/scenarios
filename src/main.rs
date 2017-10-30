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


use std::io;
use std::num::ParseIntError;

use scenarios::Scenario;
use consumers::commandline::{self, CommandLine};
use consumers::children;
use consumers::pool;


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
    let mut token_stock = consumers::TokenStock::new(max_num_tokens_from_args(args)?);
    let mut children = consumers::ProcessPool::with_capacity(token_stock.num_remaining());
    // Our handler for finished child processes.
    let reaper = |child: children::FinishedChild| if keep_going {
        Ok(())
    } else {
        child.into_result()
    };
    // Iterate over all scenarios. Because `children` panicks if we
    // drop it while it's still full, we use an anonymous function to
    // let no result escape. TODO: Wait for `catch_expr`.
    let run_result: Result<(), Error> = (|| {
        for scenario in scenarios {
            let token = pool::spin_wait_for_token(&mut token_stock, &mut children, &reaper)?;
            let child = command_line.with_scenario(scenario?)?;
            let child = child.spawn_or_return_token(token, &mut token_stock)?;
            children.push(child);
        }
        Ok(())
    })();
    // Reap all remaining children and discard their tokens.
    let finished_children = children.wait_and_reap_all().map(|(result, _)| result);
    // Here, the pool is empty and we can evaluate all possible errors.
    // I/O errors always get evaluated, exit statuses only maybe.
    run_result?;
    for child in finished_children {
        let child = child?;
        if !keep_going {
            child.into_result()?;
        }
    }
    Ok(())
}


fn handle_printing<I>(scenarios: I, args: &clap::ArgMatches) -> Result<(), Error>
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


fn max_num_tokens_from_args<'a>(args: &'a clap::ArgMatches) -> Result<usize, Error> {
    if let Some(num) = args.value_of("jobs") {
        num.parse::<usize>().map_err(Error::from)
    } else if args.is_present("jobs") {
        Ok(num_cpus::get())
    } else {
        Ok(1)
    }
}


fn command_line_from_args<'a>(args: &'a clap::ArgMatches) -> Result<CommandLine<&'a str>, Error> {
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
