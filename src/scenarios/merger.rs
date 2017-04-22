
use std::error::Error;
use std::fmt::{self, Display};

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
        where I: 'b + IntoIterator<Item = &'b Scenario>
    {
        let mut combined = MergedScenario::new();
        for scenario in scenarios.into_iter() {
            combined.merge(scenario, self.delimiter, self.strict);
        }
        combined.into_inner()
    }
}

impl<'a> Default for Merger<'a> {
    fn default() -> Self { Merger::new() }
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
            }
            Err(MergeError::NoScenarios) => {
                self.0 = Ok(other.clone());
                return;
            }
            Err(_) => {
                return;
            }
        }
        // Now that the borrow of `self` has ended, we can modify it.
        if let Err(err) = merge_result {
            self.0 = Err(MergeError::from(err));
        }
    }
}


/// Error that represents all failing modes of scenario merging.
#[derive(Debug)]
pub enum MergeError {
    /// No scenarios have been merged at all.
    NoScenarios,
    /// Scenarios have been merged, but the strict mode was violated.
    ScenarioError(ScenarioError),
}

impl Display for MergeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MergeError::NoScenarios => write!(f, "{}", self.description()),
            MergeError::ScenarioError(ref err) => err.fmt(f),
        }
    }
}

impl Error for MergeError {
    fn description(&self) -> &str {
        match *self {
            MergeError::NoScenarios => "scenario merge: no scenarios provided",
            MergeError::ScenarioError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        if let MergeError::ScenarioError(ref err) = *self {
            Some(err)
        } else {
            None
        }
    }
}

impl From<ScenarioError> for MergeError {
    fn from(err: ScenarioError) -> Self {
        MergeError::ScenarioError(err)
    }
}
