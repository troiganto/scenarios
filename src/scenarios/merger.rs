
use super::scenario::{Scenario, ScenarioError};


/// Convenience alias.
type Result = ::std::result::Result<Scenario, MergeError>;


/// A merger is a simple struct that carries part of the arguments to
/// Scenario::merge – `delimiter` and `strict`, to be exact. The reason
/// is that these arguments should be the same for all consecutive
/// calls to `merge`.
#[derive(Debug)]
pub struct Merger<'a> {
    strict: bool,
    delimiter: &'a str,
}

impl<'a> Merger<'a> {
    /// Creates a new `Merger` with default arguments.
    ///
    /// By default, `strict` mode is enabled and the `delimiter` is
    /// `", "` (comma plus space).
    pub fn new() -> Self {
        Merger {
            strict: true,
            delimiter: ", ",
        }
    }

    pub fn is_strict(&self) -> bool {
        self.strict
    }

    pub fn delimiter(&self) -> &str {
        self.delimiter
    }

    pub fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
    }

    pub fn set_delimiter<'b: 'a>(&mut self, delimiter: &'b str) {
        self.delimiter = delimiter;
    }

    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.set_strict(strict);
        self
    }

    pub fn with_delimiter<'b: 'a>(mut self, delimiter: &'b str) -> Self {
        self.set_delimiter(delimiter);
        self
    }

    /// Merges several scenarios into one.
    ///
    /// See `Scenario::merge` for more information.
    ///
    /// # Errors
    /// The merge can fail for two reasons:
    /// 1. The iterator `scenarios` was empty – in that case, the
    ///    error `MergeError::NoScenarios` is returned.
    /// 2. Strict mode was enabled and two scenarios defined the same
    ///    variable – in that case, the error
    ///    `MergeError::ScenarioError` is returned, wrapping around a
    ///    `ScenarioError::StrictMergeFailed`. The latter contains
    ///    the names of the offending scenarios and variables.
    pub fn merge<'b, I>(&self, scenarios: I) -> Result
    where
        I: 'b + IntoIterator<Item = &'b Scenario>,
    {
        let mut combined = MergedScenario::new();
        for scenario in scenarios.into_iter() {
            combined.merge(scenario, self.delimiter, self.strict);
        }
        combined.into_inner()
    }
}

impl<'a> Default for Merger<'a> {
    fn default() -> Self {
        Merger::new()
    }
}


/// A helper type for `Merger`.
///
/// It simply ensures that iteration inside `Merger::merge` does the
/// right thing.
#[derive(Debug)]
struct MergedScenario(Result);

impl MergedScenario {
    /// Creates a new instance.
    ///
    /// The initial value wrapped by this struct is
    /// `Err(MergeError::NoScenarios)`, indicating that is empty.
    fn new() -> MergedScenario {
        MergedScenario(Err(MergeError::NoScenarios))
    }

    /// Unwraps the `Result` inside `self`.
    fn into_inner(self) -> Result {
        self.0
    }

    /// Returns `true` if `merge` has not been called yet.
    ///
    /// Note that this will return `false` in two situations:
    /// 1. A `Scenario` was successfully `merge`d.
    /// 2. A call to `merge` was attempted, but failed.
    fn is_empty(&self) -> bool {
        if let Err(MergeError::NoScenarios) = self.0 {
            true
        } else {
            false
        }
    }

    /// Returns `true` if `merge` has has failed at any point.
    ///
    /// A `MergedScenario` contains a valid scenario if both
    /// `is_empty()` and `failed()` return `false`.
    fn failed(&self) -> bool {
        if let Err(MergeError::ScenarioError(_)) = self.0 {
            true
        } else {
            false
        }
    }

    /// Merges another scenario into this one.
    ///
    /// If no scenario was added before, the other scenario is simply
    /// cloned into this one. Otherwise, this scenario and the other
    /// are combined via `Scenario::merge`.
    ///
    /// #Errors
    /// If `strict` is `true` and the merge fails, `self` is set to
    /// its `Err` variant and cannot be reset.
    fn merge(&mut self, other: &Scenario, delimiter: &str, strict: bool) {
        let merge_result;
        match self.0 {
            Ok(ref mut inner) => {
                // Set merge result here, evaluate it after the borrow
                // of `inner` ends.
                merge_result = inner.merge(other, delimiter, strict);
            },
            Err(MergeError::NoScenarios) => {
                self.0 = Ok(other.clone());
                return;
            },
            Err(_) => {
                return;
            },
        }
        // Now that the borrow of `self` has ended, we can modify it.
        if let Err(err) = merge_result {
            self.0 = Err(MergeError::from(err));
        }
    }
}

impl Default for MergedScenario {
    fn default() -> Self {
        MergedScenario::new()
    }
}


quick_error! {
    /// Error that represents all failing modes of scenario merging.
    #[derive(Debug)]
    pub enum MergeError {
        /// No scenarios have been merged at all.
        NoScenarios {
            description("scenario merge: no scenarios provided")
        }
        /// Scenarios have been merged, but the strict mode was violated.
        ScenarioError(err: ScenarioError) {
            description(err.description())
            display("{}", err)
            cause(err)
            from()
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn make_scenario(name: &str, vars: &[&str]) -> Scenario {
        let mut result = Scenario::new(name).expect(name);
        for var in vars.into_iter().cloned() {
            result.add_variable(var, "").expect(var);
        }
        result
    }

    #[test]
    fn test_empty() {
        let merged = MergedScenario::new();
        assert!(merged.is_empty());
        assert!(!merged.failed());
        match merged.into_inner() {
            Err(MergeError::NoScenarios) => assert!(true),
            _ => assert!(false),
        }
    }

    #[test]
    fn test_add_one() {
        let expected = make_scenario("A", &[]);
        let mut merged = MergedScenario::new();
        merged.merge(&expected, ", ", true);

        assert!(!merged.is_empty());
        assert!(!merged.failed());

        let actual = merged.into_inner().unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_add_two() {
        let expected = make_scenario("A -- B", &["a", "b"]);
        let one = make_scenario("A", &["a"]);
        let two = make_scenario("B", &["b"]);

        let mut merged = MergedScenario::new();
        merged.merge(&one, " -- ", true);
        merged.merge(&two, " -- ", true);

        assert!(!merged.is_empty());
        assert!(!merged.failed());

        let actual = merged.into_inner().unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_add_error() {
        let one = make_scenario("A", &["a"]);
        let two = make_scenario("B", &["a"]);

        let mut merged = MergedScenario::new();
        merged.merge(&one, ", ", true);
        merged.merge(&two, ", ", true);

        assert!(!merged.is_empty());
        assert!(merged.failed());

        let err = merged.into_inner().unwrap_err();
        match err {
            MergeError::ScenarioError(ScenarioError::StrictMergeFailed {
                                          varname,
                                          left,
                                          right,
                                      }) => {
                assert_eq!(varname, "a".to_owned());
                assert_eq!(left, "A".to_owned());
                assert_eq!(right, "B".to_owned());
            },
            _ => assert!(false),
        }
    }

    #[test]
    fn test_add_non_strict() {
        let expected = make_scenario("A, B", &["a"]);
        let one = make_scenario("A", &["a"]);
        let two = make_scenario("B", &["a"]);

        let mut merged = MergedScenario::new();
        merged.merge(&one, ", ", false);
        merged.merge(&two, ", ", false);

        assert!(!merged.is_empty());
        assert!(!merged.failed());

        let actual = merged.into_inner().unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_merger() {
        let expected = make_scenario("A/B/C", &["a", "aa", "b", "bb", "c", "cc"]);
        let all = [
            make_scenario("A", &["a", "aa"]),
            make_scenario("B", &["b", "bb"]),
            make_scenario("C", &["c", "cc"]),
        ];

        let actual = Merger::new().with_delimiter("/").merge(&all).unwrap();
        assert_eq!(expected, actual);
    }
}
