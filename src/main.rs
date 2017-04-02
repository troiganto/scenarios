#![allow(dead_code)]

extern crate clap;
extern crate regex;
#[macro_use]
extern crate lazy_static;

mod errors;
mod inputline;
mod scenario;
mod scenario_file;


use clap::{Arg, App};


static LONG_EXPLANATION: &'static str =
"This program takes one or more scenario files. A scenario \
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


fn main() {
    let app = App::new("scenarios")
        .version("0.0.1")
        .author("Nico Madysa <nico.madysa@tu-dresden.de>")
        .about("Run a command line multiple times in different environments.")
        .after_help(LONG_EXPLANATION)
        .help_message("Prints detailed help information")
        .arg(
            Arg::with_name("help")
            .short("h")
            .help("Prints short help information")
            )
        .arg(
            Arg::with_name("input")
            .short("i")
            .takes_value(true)
            .number_of_values(1)
            .multiple(true)
            .help(
                "Input scenario files. If multiple files are passed, \
                all possible combinations between them are used. \
                Pass \"-\" to read from stdin. You may pass this \
                option more than once."
                )
            );
    let matches = app.clone().get_matches();

    if matches.is_present("help") {
        app.after_help("").print_help().unwrap();
        return;
    }

    let files: Vec<&str> = matches
        .values_of("input")
        .map_or(Vec::new(), |values| values.collect());
    println!("{:?}", files);
}
