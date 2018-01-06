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


use clap::{self, Arg, App, AppSettings};


/// Returns an [`App`] instance.
///
/// [`App`]: ../../clap/struct.App.html
pub fn get_app() -> clap::App<'static, 'static> {
    App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .usage("scenarios [FlAGS] [OPTIONS] <SCENARIO FILES>... [--exec <COMMAND...>]")
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
             .long("exec")
             .takes_value(true)
             .allow_hyphen_values(true)
             .min_values(1)
             .value_terminator(";")
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
             .conflicts_with("strict")
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
             .takes_value(true)
             .default_value("auto")
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


#[cfg(test)]
mod tests {
    use super::get_app;
    use clap::{AppSettings, ArgMatches, Result as ClapResult};

    trait ArgMatchesExt {
        fn values_vec_of(&self, name: &str) -> Vec<&str>;
    }

    impl<'a> ArgMatchesExt for ArgMatches<'a> {
        fn values_vec_of(&self, name: &str) -> Vec<&str> {
            self.values_of(name)
                .map(Iterator::collect)
                .unwrap_or_default()
        }
    }

    fn get_matches(args: &[&str]) -> ClapResult<ArgMatches<'static>> {
        get_app()
            .setting(AppSettings::NoBinaryName)
            .get_matches_from_safe(args)
    }


    #[test]
    fn input() {
        let matches = get_matches(&["a.ini", "b.ini"]).unwrap();
        assert_eq!(&matches.values_vec_of("input"), &["a.ini", "b.ini"]);
        assert!(get_matches(&[]).is_ok());
    }

    #[test]
    fn choose() {
        let matches = get_matches(&["--choose", "a.ini", "b.ini"]).unwrap();
        assert_eq!(&matches.values_vec_of("input"), &["b.ini"]);
        assert_eq!(matches.value_of("choose"), Some("a.ini"));
    }

    #[test]
    fn exclude() {
        assert!(get_matches(&["--exclude", "a.ini", "--choose", "b.ini", "c.ini"]).is_err());
        let matches = get_matches(&["a.ini", "--exclude", "b.ini", "c.ini"]).unwrap();
        assert_eq!(&matches.values_vec_of("input"), &["a.ini", "c.ini"]);
    }

    #[test]
    fn default_action() {
        let matches = get_matches(&[]).unwrap();
        assert!(!matches.is_present("print"));
        assert!(!matches.is_present("print0"));
        assert!(!matches.is_present("exec"));
    }

    #[test]
    fn print_no_args_suffix() {
        let matches = get_matches(&["a.ini", "--print"]).unwrap();
        assert!(matches.is_present("print"));
        assert!(matches.value_of("print").is_none());
        let matches = get_matches(&["a.ini", "--print0"]).unwrap();
        assert!(matches.is_present("print0"));
        assert!(matches.value_of("print0").is_none());
    }

    #[test]
    fn print_no_args_prefix() {
        let matches = get_matches(&["--print", "--", "a.ini"]).unwrap();
        assert!(matches.is_present("print"));
        let matches = get_matches(&["--print0", "--", "a.ini"]).unwrap();
        assert!(matches.is_present("print0"));
    }

    #[test]
    fn print_with_args_suffix() {
        let matches = get_matches(&["a.ini", "--print", "<>"]).unwrap();
        assert!(matches.is_present("print"));
        assert_eq!(matches.value_of("print"), Some("<>"));
        let matches = get_matches(&["a.ini", "--print0", "<>"]).unwrap();
        assert!(matches.is_present("print0"));
        assert_eq!(matches.value_of("print0"), Some("<>"));
    }

    #[test]
    fn print_with_args_prefix_bad() {
        assert!(get_matches(&["--print", "a.ini", "b.ini"]).is_err());
        assert!(get_matches(&["--print0", "a.ini", "b.ini"]).is_err());
    }

    #[test]
    fn print_with_args_prefix_equals() {
        let matches = get_matches(&["--print=a.ini", "b.ini"]).unwrap();
        assert_eq!(matches.value_of("print"), Some("a.ini"));
        let matches = get_matches(&["--print0=a.ini", "b.ini"]).unwrap();
        assert_eq!(matches.value_of("print0"), Some("a.ini"));
    }

    #[test]
    fn print_with_args_prefix_sep() {
        let matches = get_matches(&["--print", "a.ini", "--", "b.ini"]).unwrap();
        assert_eq!(matches.value_of("print"), Some("a.ini"));
        let matches = get_matches(&["--print0", "a.ini", "--", "b.ini"]).unwrap();
        assert_eq!(matches.value_of("print0"), Some("a.ini"));
    }

    #[test]
    fn print_with_equals_and_delim_arg() {
        let matches = get_matches(&["--print=a, b", "c.ini"]).unwrap();
        assert_eq!(matches.value_of("print"), Some("a, b"));
        let matches = get_matches(&["--print0=a, b", "c.ini"]).unwrap();
        assert_eq!(matches.value_of("print0"), Some("a, b"));
    }

    #[test]
    fn exec() {
        assert!(get_matches(&["--exec"]).is_err());
        let matches = get_matches(&["--exec", "echo", "{}"]).unwrap();
        assert_eq!(matches.values_vec_of("exec"), &["echo", "{}"]);
    }

    #[test]
    fn exec_prefix_takes_all() {
        let matches = get_matches(&["--exec", "echo", "--", "a.ini"]).unwrap();
        assert_eq!(matches.values_vec_of("exec"), &["echo", "--", "a.ini"]);
    }

    #[test]
    fn exec_prefix_terminator() {
        let matches = get_matches(&["--exec", "echo", ";", "a.ini"]).unwrap();
        assert_eq!(matches.values_vec_of("exec"), &["echo"]);
    }

    #[test]
    fn print_print0_exec_conflicts() {
        assert!(get_matches(&["a.ini", "--print", "--print0"]).is_err());
        assert!(get_matches(&["a.ini", "--print", "--exec", "echo"]).is_err());
        assert!(get_matches(&["a.ini", "--print0", "--exec", "echo"]).is_err());
        assert!(get_matches(&["a.ini", "--strict", "--lax"]).is_err());
    }

    #[test]
    fn delimiter() {
        let matches = get_matches(&["--delimiter", "/", "a.ini"]).unwrap();
        assert_eq!(matches.value_of("delimiter"), Some("/"));
    }

    #[test]
    fn delimiter_arg_required() {
        assert!(get_matches(&["--delimiter"]).is_err());
    }

    #[test]
    fn delimiter_no_default() {
        assert!(!get_matches(&[]).unwrap().is_present("delimiter"));
    }

    #[test]
    fn flags_that_require_exec() {
        assert!(get_matches(&["--keep-going"]).is_err());
        assert!(get_matches(&["--ignore-env"]).is_err());
        assert!(get_matches(&["--no-insert-name"]).is_err());
        assert!(get_matches(&["--no-export-name"]).is_err());
        assert!(get_matches(&["--keep-going", "--exec", "echo"]).is_ok());
        assert!(get_matches(&["--ignore-env", "--exec", "echo"]).is_ok());
        assert!(get_matches(&["--no-insert-name", "--exec", "echo"]).is_ok());
        assert!(get_matches(&["--no-export-name", "--exec", "echo"]).is_ok());
    }

    #[test]
    fn jobs() {
        let matches = get_matches(&["--jobs", "2", "a.ini", "b.ini", "--exec", "echo"]).unwrap();
        assert!(matches.is_present("jobs"));
        assert_eq!(matches.occurrences_of("jobs"), 1);
        assert_eq!(matches.value_of("jobs"), Some("2"));
        assert_eq!(matches.values_vec_of("input"), &["a.ini", "b.ini"]);
    }

    #[test]
    fn jobs_default() {
        let matches = get_matches(&[]).unwrap();
        assert!(matches.is_present("jobs"));
        assert_eq!(matches.occurrences_of("jobs"), 0);
        assert_eq!(matches.value_of("jobs"), Some("auto"));
    }

    #[test]
    fn jobs_no_arg_required() {
        let matches = get_matches(&["--jobs", "--exec", "echo"]).unwrap();
        assert!(matches.is_present("jobs"));
        assert_eq!(matches.occurrences_of("jobs"), 1);
        assert_eq!(matches.value_of("jobs"), Some("auto"));
    }

    #[test]
    fn jobs_empty_value_allowed() {
        assert!(get_matches(&["--jobs", ""]).is_ok());
    }

    #[test]
    fn jobs_no_exec_required() {
        assert!(get_matches(&["--jobs", "2"]).is_ok());
    }

}
