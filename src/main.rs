#![allow(dead_code)]

#[macro_use]
extern crate clap;
extern crate regex;
#[macro_use]
extern crate lazy_static;

mod scenarios;
mod cartesian;


use std::fmt::Display;
use clap::{Arg, App};


static LONG_EXPLANATION: &'static str = "\
This program takes one or more scenario files. A scenario \
is named set of environment variables to apply at the same \
time. A scenario file contains a list of scenarios in the \
following format:

    [First scenario name]
    FIRST_VARIABLE = value
    SECOND_VARIABLE = value

    [Second scenario name]
    FIRST_VARIABLE = value
    SECOND_VARIABLE = value

If you pass several scenario files, all possible \
combinations that take one scenario from each file are \
executed. For instance, assume you have the following \
scenario files:

    - `numbers.ini` with scenarios named \"1\", \"2\", and \"3\";
    - `letters.ini` with scenarios named \"a\" and \"b\";

Then, the following call:

    scenarios -i numbers.ini -i letters.ini some_program

will execute the following six scenario combinations: \
\"1, a\"; \"1, b\"; \"2, a\"; \"2, b\"; \"3, a\"; and \
\"3, b\".

It is an error if two files define the same scenario name, \
or if two scenarios from different files define the same \
environment variable. This check can be disabled by passing \
the --lax option. In that case, later definitions of \
variables will overwrite former definitions. \

After reading the scenario files, the remainder of the \
command line, noted above as `...`, is executed once for \
each combination of scenarios. This may be parallelized by \
passing the --jobs option.

When running, scenarios adds an additional variable \
SCENARIOS_NAME to each scenario. This variable contains the \
name of the current combination of scenarios. Strict mode \
will prevent you from defining SCENARIOS_NAME yourself. \
With the --lax option, your own definition will silently be \
overwritten.

When using the --include argument, consider passing it as

    scenarios --include=PATTERN ...

(with an equal sign). Otherwise, your shell might expand \
the pattern before scenarios gets to see it.";


fn try_main<'a>(matches: clap::ArgMatches<'a>) -> Result<(), Box<Display>> {
    // Collect scenario file names.
    let scenario_files = matches
        .values_of("input")
        .ok_or_else(|| Box::new("no scenarios provided") as Box<Display>)?
        .map(scenarios::from_file)
        .collect::<Result<Vec<Vec<scenarios::Scenario>>, _>>()
        .map_err(|err| Box::new(err) as Box<Display>)?;

    println!("{:?}", scenario_files);
    Ok(())
}

fn main() {
    let app = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .after_help(LONG_EXPLANATION)
        .help_message("Prints detailed help information")
        .arg(Arg::with_name("short_help")
                 .short("h")
                 .help("Prints short help information"))
        .arg(Arg::with_name("input")
                 .short("i")
                 .takes_value(true)
                 .number_of_values(1)
                 .multiple(true)
                 .help("Input scenario files. If multiple files are passed, \
                all possible combinations between them are used. \
                Pass \"-\" to read from stdin. You may pass this \
                option more than once."));

    // We clone `app` here because `get_matches` consumes it -- but we
    // might still need it to print the short help!
    let matches = app.clone().get_matches();

    // If -h was passed, reduce the help message to nothing and print
    //it.
    if matches.is_present("short_help") {
        app.after_help("").print_help().unwrap();
        return;
    }

    if let Err(err) = try_main(matches) {
        let msg = err.to_string();
        let kind = clap::ErrorKind::Format;
        let err = clap::Error::with_description(&msg, kind);
        err.exit();
    }
}
