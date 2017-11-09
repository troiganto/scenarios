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
use consumers::commandline::{self, CommandLine};
use consumers::children;
use consumers::pool;


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
        CommandLineHandler::new(&args)
            .handle(combined_scenarios)?;
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
    /// Flag read from --jobs.
    is_parallel: bool,
    /// The command line that is executed for each scenario.
    command_line: consumers::CommandLine<&'a str>,
    /// Our stock of tokens for parallelism.
    tokens: consumers::TokenStock,
    /// A pool of currently-running processes.
    children: consumers::ProcessPool,
    /// A logger that helps us print information to the user.
    logger: logger::Logger<'static>,
}

impl<'a> CommandLineHandler<'a> {
    /// Creates a new handler.
    ///
    /// This reads the parsed command-line arguments and initializes
    /// the fields of this struct from them.
    pub fn new(args: &'a clap::ArgMatches) -> Self {
        let max_num_tokens = Self::max_num_tokens_from_args(args);
        CommandLineHandler {
            keep_going: args.is_present("keep_going"),
            is_parallel: max_num_tokens > 1,
            command_line: Self::command_line_from_args(args),
            tokens: consumers::TokenStock::new(max_num_tokens),
            children: consumers::ProcessPool::new(),
            logger: logger::Logger::new(args.is_present("quiet")),
        }
    }

    /// Creates a `CommandLine` from `args`.
    fn command_line_from_args(args: &'a clap::ArgMatches) -> CommandLine<&'a str> {
        let options = commandline::Options {
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
        if let Some(num) = args.value_of("jobs") {
            // We can unwrap here because clap validates --jobs for us.
            num.parse::<usize>().unwrap()
        } else if args.is_present("jobs") {
            num_cpus::get()
        } else {
            1
        }
    }

    /// Runs the main loop of the program and tears it down afterwards.
    ///
    /// This immediately calls `inner_loop()` and afterwards waits for
    /// all running child processes.
    ///
    /// # Errors
    /// Same as `inner_loop()`. While all errors are reported via
    /// `self.logger`, only the first error is returned.
    pub fn handle<'s, I>(&mut self, scenarios: I) -> Result<(), Error>
    where
        I: Iterator<Item = scenarios::Result<Scenario<'s>>>,
    {
        // If `inner_loop()` returns `Ok`, it means the pool is empty. If it
        // returns `Err`, we must clean up the pool ourselves.
        if self.inner_loop(scenarios).is_ok() {
            Ok(())
        } else {
            if self.is_parallel && !self.children.is_empty() {
                self.logger
                    .log("waiting for unfinished child processes ...")
            }
            while let Some((child, token)) = self.children.wait_reap_one() {
                self.tokens.return_token(token);
                // Coalesce `WaitError` and `ChildFailed` errors.
                if let Err(err) = child.and_then(children::FinishedChild::into_result) {
                    self.logger.log(err);
                }
            }
            // Note that we log all errors, but only return a generic "not
            // everything was successful". This avoids the same message being
            // printed twice.
            Err(Error::NotAllFinished)
        }
    }

    /// The main loop of the program.
    ///
    /// This starts one child process for each scenario and pushes it
    /// into the pool `children`. If the maximum allowed number of
    /// child processes is running, this loop waits until one of them
    /// has finished.
    ///
    /// On the happy path, this function waits until all child
    /// processes have terminated successfully and the pool
    /// `self.children` is empty again. However, if an error occurs
    /// that `inner_loop` cannot ignore, it returns immediately. In
    /// that case, `self.children` may still contain child processes
    /// and it is the task of the caller to clean them up.
    ///
    /// # Errors
    /// This function may fail if:
    /// - spawning a child process gives an `io::Error`;
    /// - waiting on a child process gives an `io::Error`;
    /// - two variable names conflict and strict mode is enabled;
    /// - a child process exits with non-zero exit status and
    ///   `keep_going` is `false`.
    fn inner_loop<'s, I>(&mut self, scenarios: I) -> Result<(), Error>
    where
        I: Iterator<Item = scenarios::Result<Scenario<'s>>>,
    {
        // If --keep-going is set, we still need a way to signal that something
        // went wrong. So we use this variable as a flag.
        let mut ignored_errors = Ok(());
        // This extra scope limits the lifetime of the grim `reaper`.
        {
            // These cumbersome bindings avoid borrowing `self` to the closure. The
            // closure itself is straightforward: If usually pass everything
            // through. But if an error occurs and --keep-going is set, we put the
            // error aside and pretend nothing happened.
            let keep_going = self.keep_going;
            let logger = &self.logger;
            let mut reaper = |child: children::FinishedChild| match child.into_result() {
                Ok(()) => Ok(()),
                Err(err) => {
                    logger.log(&err);
                    if keep_going {
                        ignored_errors = Err(Error::NotAllFinished);
                        Ok(())
                    } else {
                        Err(err)
                    }
                },
            };
            // The main loop!
            for scenario in scenarios {
                let scenario = scenario?;
                let token =
                    pool::spin_wait_for_token(&mut self.tokens, &mut self.children, &mut reaper)?;
                let child = self.command_line
                    .with_scenario(scenario)?
                    .spawn_or_return_token(token, &mut self.tokens)?;
                self.children.push(child);
            }
            // No fatal error so far, let's clean up the child process pool!
            while let Some((child, token)) = self.children.wait_reap_one() {
                self.tokens.return_token(token);
                reaper(child?)?;
            }
        }
        // If we got here, our result would usually be `Ok(())` -- unless there
        // are errors we ignored due to --keep-going!
        ignored_errors
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
        VariableNameError(err: commandline::VariableNameError) {
            description(err.description())
            display("error: {}", err)
            cause(err)
            from()
        }
        ChildError(err: children::Error) {
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
