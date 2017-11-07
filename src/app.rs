use clap::{self, Arg, ArgGroup, App, AppSettings};


pub fn get_app() -> clap::App<'static, 'static> {
    App::new(crate_name!())
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
        .arg(Arg::with_name("quiet")
             .short("q")
             .long("quiet")
             .help("Suppress information when executing commands.")
             .long_help("Suppress information during execution of \
                         commands. Errors found in the given scenario \
                         files are still printed to stderr."))
        .arg(Arg::with_name("delimiter")
             .short("d")
             .long("delimiter")
             .takes_value(true)
             .value_name("STRING")
             .default_value(", ")
             // We print the default ourselves because the by default
             // (hahah), no quotes are printed around the value.
             .hide_default_value(true)
             .help("The delimiter to use when combining scenario \
                    names. [default: ', ']"))
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
        // Input control.
        .arg(Arg::with_name("input")
             .takes_value(true)
             .value_name("SCENARIO FILES")
             .multiple(true)
             .help("The scenario files to process.")
             .long_help("The scenario files to process. If multiple \
                         files are passed, all possible combinations \
                         between them are iterated. Pass '-' to read \
                         from stdin."))
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
             .value_name("FORMAT")
             .min_values(0)
             .max_values(1)
             .help("Like --print, but separate scenario names with a \
                    null byte instead of a newline.")
             .long_help("Like --print, but separate scenario names \
                         with a null byte instead of a newline. This \
                         is useful when piping the names to \
                         \"xargs -0\"."))
        // Command line execution.
        .arg(Arg::with_name("command_line")
             .takes_value(true)
             .value_name("COMMAND")
             .multiple(true)
             .last(true)
             .help("A command line to execute for each scenario \
                    combination.")
             .long_help("A command line to execute for each scenario \
                         combination. This must always preceded by \
                         \"--\" to distinguish it from the list of \
                         scenario files."))
        .arg(Arg::with_name("ignore_env")
             .short("I")
             .long("ignore-env")
             .requires("command_line")
             .help("Don't export the current environment to COMMAND.")
             .long_help("Don't export the current environment to \
                         COMMAND. If this flag is passed, COMMAND sees \
                         _only_ the environment variables defined in \
                         the scenario files."))
        .arg(Arg::with_name("no_insert_name")
             .long("no-insert-name")
             .requires("command_line")
             .help("Don't replace '{}' with SCENARIOS_NAME when \
                    reading COMMAND."))
        .arg(Arg::with_name("no_export_name")
             .long("no-export-name")
             .requires("command_line")
             .help("Don't export SCENARIOS_NAME to COMMAND.")
             .long_help("Don't export SCENARIOS_NAME to COMMAND. If \
                         use this parameter, you are able to define \
                         your own SCENARIOS_NAME without it being \
                         overwritten. (Why would you, though?)"))
        .arg(Arg::with_name("keep_going")
             .short("k")
             .long("keep-going")
             .requires("command_line")
             .help("Don't abort if a COMMAND fails.")
             .long_help("Don't abort if a COMMAND fails. The default \
                         is to cancel everything as soon as one child \
                         processes has been found out to have failed."))
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
            .help("The number of COMMANDs to execute in parallel.")
            .long_help("The number of COMMANDs to execute in parallel. \
                       If no number is passed, the detected number of \
                       CPUs on this machine is used."))
}


pub fn print_short_help(app: clap::App) {
    app.after_help("").print_help().unwrap();
}

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
