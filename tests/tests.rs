
mod runner;

use runner::Runner;


#[test]
fn test_no_args() {
    let expected = "error: no scenarios provided\n";
    let output = Runner::new().output();
    assert_eq!(expected, &output.stderr);
    assert_eq!("", &output.stdout);
    assert!(!output.status.success());
}


#[test]
fn test_one_scenario() {
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
fn test_print_template() {
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
fn test_default_mode_is_strict() {
    let output = Runner::new()
        .scenario_files(&["good_a.ini", "conflicts_with_a.ini"])
        .output();
    assert!(!output.status.success());
}
