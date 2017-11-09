
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
        let expected_stderr = "error: variable \"a_var1\" defined both in scenario \"A1\" and in \
                               scenario \"C3\"\n";
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
}

mod environment {
    use runner::Runner;

    #[test]
    fn test_insert_name() {
        let expected = "-A1-\n-A2-\n";
        let output = Runner::new()
            .scenario_file("good_a.ini")
            .args(&["--", "echo", "-{}-"])
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
            .args(&["--", "echo", "-{}-"])
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
            .args(&["--", "env"])
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
            .args(&["--", "env"])
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
            .args(&["--", "env"])
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
            .args(&["--", "env"])
            .output();
        assert_eq!("", &output.stderr);
        assert_eq!(expected, &output.stdout);
        assert!(output.status.success());
    }
}

mod errors {
    use runner::Runner;


    ///
    fn stop_at_scenario(name: &str, additional_args: &[&str]) -> Runner {
        let script = format!("if [ {{}} = {} ]; then exit 1; else exit 0; fi", name);
        let mut runner = Runner::new();
        runner
            .scenario_file("many_scenarios.ini")
            .args(additional_args);
            .args(&["--", "sh", "-c", &script]);
        runner
    }

    #[test]
    fn test_no_args() {
        let expected = "error: no scenarios provided\n";
        let output = Runner::new().output();
        assert_eq!(expected, &output.stderr);
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_missing_file() {
        let output = Runner::new()
            .scenario_file("does not exist")
            .output();
        assert_eq!("", &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_stop_at_first_error() {
        let expected_stderr = "scenarios: command returned non-zero exit code: 1\n\tin scenario \"3\"\n";
        let expected_stdout = "error: no scenarios provided\n";
        let output = stop_at_scenario("3", &[])
            .output();
        assert_eq!(expected_stdout, &output.stderr);
        assert_eq!(expected_stderr, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_stop_at_first_error_parallel() {
        let expected_stderr = "";
        let expected_stdout = "";
        let output = stop_at_scenario("1", &["--jobs=3"])
            .output();
        assert_eq!(expected_stdout, &output.stderr);
        assert_eq!(expected_stderr, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_finish_what_is_started() {
        let expected_stderr = "";
        let expected_stdout = "";
        let output = Runner::new()
            .args(&["--jobs=3", "--", "sh" , "-c", "exit 1"])
            .output();
        assert_eq!(expected_stdout, &output.stderr);
        assert_eq!(expected_stderr, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_keep_going() {
        let expected_stderr = "";
        let expected_stdout = "";
        let output = stop_at_scenario("1", &["--keep-going"])
            .output();
        assert_eq!(expected_stdout, &output.stderr);
        assert_eq!(expected_stderr, &output.stdout);
        assert!(!output.status.success());
    }


    #[test]
    fn test_keep_going_parallel() {
        let expected_stderr = "";
        let expected_stdout = "";
        let output = stop_at_scenario("1", &["--keep-going", "--jobs=3"])
            .output();
        assert_eq!(expected_stdout, &output.stderr);
        assert_eq!(expected_stderr, &output.stdout);
        assert!(!output.status.success());
    }
}
