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


mod runner;

mod printing {
    use runner::Runner;


    #[test]
    fn test_simple() {
        let expected = "A1\nA2\n";
        let output = Runner::new().scenario_file("good_a.ini").output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_delimiter() {
        let expected = "A1 -- B1\nA1 -- B2\nA2 -- B1\nA2 -- B2\n";
        let output = Runner::new()
            .arg("-d -- ")
            .scenario_files(&["good_a.ini", "good_b.ini"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_print() {
        let expected = "A1\nA2\n";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--print")
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_print0() {
        let expected = "A1\0A2\0";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--print0")
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_template() {
        let expected = "Some(A1)\nSome(A2)\n";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .args(&["--print", "Some({})"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_lax_mode() {
        let expected = "A1, C1\nA1, C2\nA1, C3\nA2, C1\nA2, C2\nA2, C3\n";
        let output = Runner::new()
            .arg("--lax")
            .scenario_files(&["good_a.ini", "conflicts_with_a.ini"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_strict_mode() {
        let expected_stdout = "A1, C1\nA1, C2\n";
        let expected_stderr = "scenarios: error: variable \"a_var1\" defined both in scenario \
                               \"A1\" and in scenario \"C3\"\n";
        let output = Runner::new()
            .arg("--strict")
            .scenario_files(&["good_a.ini", "conflicts_with_a.ini"])
            .output();
        assert_eq!(expected_stderr, &output.stderr);
        assert_eq!(expected_stdout, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_strict_mode_is_default() {
        let output = Runner::new()
            .scenario_files(&["good_a.ini", "conflicts_with_a.ini"])
            .output();
        assert!(!output.status.success());
    }


    #[test]
    fn test_choose() {
        let expected = "A1\n";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .args(&["--choose", "?1"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }

    #[test]
    fn test_exclude() {
        let expected = "1\n3\n5\n";
        let output = Runner::new()
            .scenario_file("many_scenarios.ini")
            .args(&["--exclude", "[24]"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }
}

mod environment {
    use runner::Runner;

    #[test]
    fn test_insert_name() {
        let expected = "-A1-\n-A2-\n";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .args(&["--exec", "echo", "-{}-"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_no_insert_name() {
        let expected = "-{}-\n-{}-\n";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--no-insert-name")
            .args(&["--exec", "echo", "-{}-"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_no_export_name() {
        let expected = "outer_variable=1\n";
        let output = Runner::new()
            .scenario_file("one_empty.ini")
            .arg("--no-export-name")
            .args(&["--exec", "env"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_ignore_env() {
        let expected = "SCENARIOS_NAME=Empty\n";
        let output = Runner::new()
            .scenario_file("one_empty.ini")
            .arg("--ignore-env")
            .args(&["--exec", "env"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_empty_env() {
        let output = Runner::new()
            .scenario_file("one_empty.ini")
            .args(&["--ignore-env", "--no-export-name"])
            .args(&["--exec", "env"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(output.status.success());
    }


    #[test]
    fn test_non_empty_env() {
        let expected = "a_var1=This conflicts with A1 and A2.\n";
        let output = Runner::new()
            .scenario_file("conflicts_with_a.ini")
            .args(&["--ignore-env", "--no-export-name"])
            .args(&["--exec", "env"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }
}

mod errors {
    use runner::Runner;


    /// Returns a runner that will fail in a specific scenario.
    fn stop_at_scenario(name: &str, additional_args: &[&str]) -> Runner {
        let script = format!("if [ {{}} = {} ]; then exit 1; else echo {{}}; fi", name);
        let mut runner = Runner::new();
        runner
            .scenario_file("many_scenarios.ini")
            .args(additional_args)
            .args(&["--exec", "sh", "-c", &script]);
        runner
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn test_conflict_print_exec() {
        let mut runner = Runner::new();
        runner.args(&["--print", "--exec", "echo", "aaa"]);
        let expected = "error: The argument '--exec <COMMAND...>' cannot be used with '--print \
                        <FORMAT>'

USAGE:
    scenarios [FlAGS] [OPTIONS] <SCENARIO FILES>... [--exec <COMMAND...>]

For more information try --help
";
        let output = runner.output();
        assert_eq!(&expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn test_conflict_print0_exec() {
        let mut runner = Runner::new();
        runner.args(&["--print0", "--exec", "echo", "aaa"]);
        let expected = "error: The argument '--exec <COMMAND...>' cannot be used with '--print0 \
                        <FORMAT>'

USAGE:
    scenarios [FlAGS] [OPTIONS] <SCENARIO FILES>... [--exec <COMMAND...>]

For more information try --help
";
        let output = runner.output();
        assert_eq!(&expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn test_conflict_print_print0() {
        let mut runner = Runner::new();
        runner.args(&["--print", "{}", "--print0", "{}"]);
        let expected = "error: The argument '--print0 <FORMAT>' cannot be used with '--print \
                        <FORMAT>'

USAGE:
    scenarios [FlAGS] [OPTIONS] <SCENARIO FILES>... [--exec <COMMAND...>]

For more information try --help
";
        let output = runner.output();
        assert_eq!(&expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_no_args() {
        let expected = "scenarios: error: no scenarios provided\n";
        let output = Runner::new().output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_missing_file() {
        let output = Runner::new().arg("does not exist").output();
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_broken_file() {
        let mut runner = Runner::new();
        runner.scenario_file("broken.ini");
        let expected = format!(
            r#"scenarios: error: could not read file
scenarios:   -> reason: in {0}:1
scenarios:   -> reason: in {0}:17
scenarios:   -> reason: duplicate scenario name: "Scenario 1"
"#,
            runner.get_scenario_file_path("broken.ini").display()
        );
        let output = runner.output();
        assert_eq!(&expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_broken_command() {
        let expected = r#"scenarios: error: could not start scenario "A1"
scenarios:   -> reason: could not execute command "not a command"
scenarios:   -> reason: No such file or directory (os error 2)
scenarios: not all scenarios terminated successfully
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .args(&["--exec", "not a command"])
            .output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_broken_command_parallel() {
        let expected = r#"scenarios: error: could not start scenario "A1"
scenarios:   -> reason: could not execute command "not a command"
scenarios:   -> reason: No such file or directory (os error 2)
scenarios: waiting for unfinished jobs ...
scenarios: not all scenarios terminated successfully
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .args(&["--jobs=2", "--exec", "not a command"])
            .output();
        assert_eq!(expected, &output.stderr);
        assert!(!output.status.success());
    }


    #[test]
    fn test_stop_at_first_error() {
        let expected_stderr = r#"scenarios: error: scenario did not finish successfully: "3"
scenarios:   -> reason: job exited with non-zero exit code: 1
scenarios: not all scenarios terminated successfully
"#;
        let expected_stdout = "1\n2\n";
        let output = stop_at_scenario("3", &[]).output();
        assert_eq!(expected_stderr, &output.stderr);
        assert_eq!(expected_stdout, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_stop_at_first_error_parallel() {
        let expected_stderr = r#"scenarios: error: scenario did not finish successfully: "1"
scenarios:   -> reason: job exited with non-zero exit code: 1
scenarios: waiting for unfinished jobs ...
scenarios: not all scenarios terminated successfully
"#;
        let expected_stdout = "2\n3\n";
        let output = stop_at_scenario("1", &["--jobs=3"]).output();
        assert_eq!(expected_stderr, &output.stderr);
        assert_eq!(expected_stdout, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_finish_what_is_started() {
        let expected_stderr = r#"scenarios: error: scenario did not finish successfully: "1"
scenarios:   -> reason: job exited with non-zero exit code: 1
scenarios: waiting for unfinished jobs ...
scenarios: error: scenario did not finish successfully: "2"
scenarios:   -> reason: job exited with non-zero exit code: 1
scenarios: not all scenarios terminated successfully
"#;
        let expected_stdout = "";
        let output = Runner::new()
            .scenario_file("many_scenarios.ini")
            .args(&["--jobs=2", "--exec", "sh", "-c", "exit 1"])
            .output();
        assert_eq!(expected_stderr, &output.stderr);
        assert_eq!(expected_stdout, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_keep_going() {
        let expected_stderr = r#"scenarios: error: scenario did not finish successfully: "1"
scenarios:   -> reason: job exited with non-zero exit code: 1
scenarios: not all scenarios terminated successfully
"#;
        let expected_stdout = "2\n3\n4\n5\n";
        let output = stop_at_scenario("1", &["--keep-going"]).output();
        assert_eq!(expected_stderr, &output.stderr);
        assert_eq!(expected_stdout, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_keep_going_parallel() {
        let expected_stderr = r#"scenarios: error: scenario did not finish successfully: "1"
scenarios:   -> reason: job exited with non-zero exit code: 1
scenarios: not all scenarios terminated successfully
"#;
        let expected_stdout = "2\n3\n4\n5\n";
        let output = stop_at_scenario("1", &["--keep-going", "--jobs=3"]).output();
        assert_eq!(expected_stderr, &output.stderr);
        assert_eq!(expected_stdout, &output.stdout);
        assert!(!output.status.success());
    }
}

mod invalid_args {
    use runner::Runner;
    use runner::OsStringExt;
    use std::ffi::OsString;


    #[test]
    fn test_delimiter() {
        let expected = r#"scenarios: error: invalid value for --delimiter
scenarios:   -> reason: contains invalid UTF-8 character: "�"
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--delimiter")
            .arg(OsString::from_bytes(b"\xFA"))
            .output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_input_file() {
        // Here we check that a non-UTF8 filename does not cause a panic.
        let expected_first_line = "scenarios: error: could not read file";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg(OsString::from_bytes(b"broken_name_\xFA.ini"))
            .output();
        let first_line = output.stderr.lines().next().unwrap();
        assert_eq!(expected_first_line, first_line);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_choose() {
        let expected = r#"scenarios: error: invalid value for --choose
scenarios:   -> reason: contains invalid UTF-8 character: "n�f"
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--choose")
            .arg(OsString::from_bytes(b"n\xFAf"))
            .output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_exclude() {
        let expected = r#"scenarios: error: invalid value for --exclude
scenarios:   -> reason: contains invalid UTF-8 character: "n�f"
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--exclude")
            .arg(OsString::from_bytes(b"n\xFAf"))
            .output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_print() {
        let expected = r#"scenarios: error: invalid value for --print
scenarios:   -> reason: contains invalid UTF-8 character: "n�f"
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--print")
            .arg(OsString::from_bytes(b"n\xFAf"))
            .output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_print0() {
        let expected = r#"scenarios: error: invalid value for --print0
scenarios:   -> reason: contains invalid UTF-8 character: "n�f"
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--print0")
            .arg(OsString::from_bytes(b"n\xFAf"))
            .output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_command_line() {
        // Here we check that a non-UTF8 command does not cause a panic.
        let expected_first_line = "scenarios: error: could not start scenario \"A1\"";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--exec")
            .arg(OsString::from_bytes(b"ec\xFAo"))
            .output();
        let first_line = output.stderr.lines().next().unwrap();
        assert_eq!(expected_first_line, first_line);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_jobs_no_unicode() {
        let expected = r#"scenarios: error: invalid value for --jobs
scenarios:   -> reason: contains invalid UTF-8 character: "n�f"
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .arg("--jobs")
            .arg(OsString::from_bytes(b"n\xFAf"))
            .args(&["--exec", "echo"])
            .output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_jobs_not_a_number() {
        let expected = r#"scenarios: error: invalid value for --jobs
scenarios:   -> reason: not a number: "three"
"#;
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .args(&["--jobs", "three", "--exec", "echo"])
            .output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }
}
