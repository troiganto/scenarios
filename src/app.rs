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


//! Contains all calls to `clap` so it doesn't clutter `main()`.


use clap::{self, Arg, ArgGroup, App, AppSettings};


/// Returns an [`App`] instance.
///
/// [`App`]: ../../clap/struct.App.html
pub fn get_app() -> clap::App<'static, 'static> {
    App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .usage("scenarios [FlAGS] [OPTIONS] <SCENARIO FILES>... [-- <COMMAND>...]")
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
        .arg(Arg::with_name("quiet")
             .short("q")
             .long("quiet")
             .help("Suppress information when executing commands.")
             .long_help("Suppress information during execution of \
                         commands. Errors found in the given scenario \
                         files are still printed to stderr."))

        // Main options.
        .arg(Arg::with_name("print")
             .long("print")
             .takes_value(true)
             .min_values(0)
             .max_values(1)
             .value_name("FORMAT")
             .help("Print SCENARIOS_NAME to stdout for each scenario \
                    combination. [default]")
             .long_help("Print SCENARIOS_NAME to stdout for each \
                         scenario combination. Names are separated by \
                         newlines. An optional format string may be \
                         passed, in which \"{}\" gets replaced with \
                         SCENARIOS_NAME. [default]"))
        .arg(Arg::with_name("print0")
             .long("print0")
             .takes_value(true)
             .min_values(0)
             .max_values(1)
             .conflicts_with("print")
             .value_name("FORMAT")
             .help("Like --print, but separate scenario names with a \
                    null byte instead of a newline.")
             .long_help("Like --print, but separate scenario names \
                         with a null byte instead of a newline. This \
                         is useful when piping the names to \
                         \"xargs -0\"."))
        .arg(Arg::with_name("exec")
             .takes_value(true)
             .multiple(true)
             .last(true)
             .conflicts_with("print")
             .conflicts_with("print0")
             .value_name("COMMAND...")
             .help("A command line to execute for each scenario \
                    combination.")
             .long_help("A command line to execute for each scenario \
                         combination. This must always preceded by \
                         \"--\" to distinguish it from the list of \
                         scenario files."))

        // Input control.
        .arg(Arg::with_name("input")
             .takes_value(true)
             .multiple(true)
             .value_name("SCENARIO FILES")
             .help("The scenario files to process.")
             .long_help("The scenario files to process. If multiple \
                         files are passed, all possible combinations \
                         between them are iterated. Pass '-' to read \
                         from stdin."))
        .arg(Arg::with_name("choose")
             .short("c")
             .long("choose")
             .takes_value(true)
             .value_name("SCENARIO NAME")
             .help("Only process scenarios with the given name.")
             .long_help("Ignore all scenarios except the one with the \
                         given name. SCENARIO NAME may be a \
                         shell-like glob pattern to choose more than \
                         one scenario at once."))
        .arg(Arg::with_name("exclude")
             .short("x")
             .long("exclude")
             .takes_value(true)
             .conflicts_with("choose")
             .value_name("SCENARIO NAME")
             .help("Ignore scenarios with the given name.")
             .long_help("Ignore all scenarios with the given name. As \
                         for --choose, SCENARIO NAME may be a \
                         shell-like glob pattern."))

        // Strict mode control.
        .group(ArgGroup::with_name("strict_mode")
               .args(&["strict", "lax"])
               .required(false))
        .arg(Arg::with_name("strict")
             .short("s")
             .long("strict")
             .help("Produce error on conflicting definitions of \
                    environment variables. [default]")
             .long_help("Produce error on conflicting definitions of \
                         environment variables. No two scenario files \
                         may define the same scenario name or \
                         environment variable. You may not define a \
                         variable called \"SCENARIOS_NAME\" unless \
                         --no-export-name is passed. [default]"))
        .arg(Arg::with_name("lax")
             .short("l")
             .long("lax")
             .help("Disable strict mode."))

        // Command line execution.
        .arg(Arg::with_name("ignore_env")
             .short("I")
             .long("ignore-env")
             .requires("exec")
             .help("Don't export the current environment to COMMAND.")
             .long_help("Don't export the current environment to \
                         COMMAND. If this flag is passed, COMMAND sees \
                         _only_ the environment variables defined in \
                         the scenario files."))
        .arg(Arg::with_name("no_insert_name")
             .long("no-insert-name")
             .requires("exec")
             .help("Don't replace '{}' with SCENARIOS_NAME when \
                    reading COMMAND."))
        .arg(Arg::with_name("no_export_name")
             .long("no-export-name")
             .requires("exec")
             .help("Don't export SCENARIOS_NAME to COMMAND.")
             .long_help("Don't export SCENARIOS_NAME to COMMAND. If \
                         use this parameter, you are able to define \
                         your own SCENARIOS_NAME without it being \
                         overwritten. (Why would you, though?)"))

        // Handling multiple scenarios.
        .arg(Arg::with_name("delimiter")
             .short("d")
             .long("delimiter")
             .takes_value(true)
             .default_value(", ")
             .hide_default_value(true)
             .value_name("STRING")
             .help("The delimiter to use when combining scenario \
                    names. [default: ', ']"))
        .arg(Arg::with_name("keep_going")
             .short("k")
             .long("keep-going")
             .requires("exec")
             .help("Don't abort if a COMMAND fails.")
             .long_help("Don't abort if a COMMAND fails. The default \
                         is to cancel everything as soon as one job \
                         has been found out to have failed."))
        .arg(Arg::with_name("jobs")
             .short("j")
             .long("jobs")
             .requires("exec")
             .takes_value(true)
             .min_values(0)
             .max_values(1)
             .value_name("N")
             .help("The number of COMMANDs to execute in parallel.")
             .long_help("The number of COMMANDs to execute in \
                        parallel. If no number is passed, the detected \
                        number of CPUs on this machine is used."))
}


/// Prints the information given by the `-h` argument.
pub fn print_short_help(app: clap::App) {
    app.after_help("").print_help().unwrap();
}

/// Prints the information given by the `--help` argument.
pub fn print_long_help(app: clap::App) {
    app.after_help(LONG_EXPLANATION)
        .print_long_help()
        .unwrap();
    print!("\n\n");
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
each scenario (unless --no-export-name is passed). This variable \
contains the name of the current combination of scenarios. Strict \
mode will prevent you from defining SCENARIOS_NAME yourself. With the \
--lax option, your own definition will silently be overwritten.
";
