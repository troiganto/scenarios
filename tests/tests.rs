
mod runner;

use runner::Runner;


#[test]
fn test_no_args() {
    let expected = "error: no scenarios provided\n";
    let output = Runner::new().output();
    let actual = String::from_utf8_lossy(&output.stderr);
    assert_eq!(expected, &actual);
}
