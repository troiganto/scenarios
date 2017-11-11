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


use std::error::Error;
use std::iter::FromIterator;
use std::fmt::{self, Display};
use std::borrow::{Borrow, Cow};
use std::collections::hash_map::{self, HashMap};

use quick_error::ResultExt;


/// Convenience alias for `std::result::Result`.
pub type Result<T> = ::std::result::Result<T, ScenarioError>;


/// Named set of environment variable definitions.
///
/// A scenario has a name and a set of environment variable definitions.
/// Each definition has an associated variable name and the
/// corresponding
/// value, both strings. A variable name must follow the rules for
/// regular
/// C identifiers. A scenario name must be non-empty and not contain
/// any null byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario<'a> {
    name: Cow<'a, str>,
    variables: HashMap<&'a str, &'a str>,
}

impl<'a> Scenario<'a> {
    /// Creates a new scenario named `name`.
    ///
    /// # Errors
    /// This call fails with `ParseError::InvalidName` if `name`
    /// is the empty string or contains a null byte.
    pub fn new<S: Into<Cow<'a, str>>>(name: S) -> Result<Self> {
        let name = name.into();
        if name.is_empty() || name.contains('\0') {
            Err(ScenarioError::InvalidName(name.into_owned()))
        } else {
            let variables = HashMap::new();
            Ok(Scenario { name, variables })
        }
    }

    /// Adds another variable definition of the current set.
    ///
    /// # Errors
    /// This call fails with `ScenarioError::InvalidVariable` if `name`
    /// is not a valid variable name (`[A-Za-z_][A-Za-z0-9_]+`). It
    /// fails with `ScenarioError::DuplicateVariable` if a variable of
    /// this name already has been added to the scenario.
    pub fn add_variable(&mut self, name: &'a str, value: &'a str) -> Result<()> {
        if self.has_variable(name) {
            Err(ScenarioError::DuplicateVariable(name.to_owned()))
        } else if !is_c_identifier(name) {
            Err(ScenarioError::InvalidVariable(name.to_owned()))
        } else {
            self.variables.insert(name, value);
            Ok(())
        }
    }

    /// Returns the name of the scenario.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns `true` if the variable already exists in this scenario.
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }

    /// Returns the value of variable named `name`, if it exists.
    pub fn get_variable(&self, name: &str) -> Option<&'a str> {
        self.variables.get(name).cloned()
    }

    /// Returns an iterator over all variable names.
    pub fn variable_names(&self) -> hash_map::Keys<&'a str, &'a str> {
        self.variables.keys()
    }

    /// Returns an iterator over all variables.
    pub fn variables(&self) -> hash_map::Iter<&'a str, &'a str> {
        self.variables.iter()
    }

    /// Consumes the scenario to return an iterator over all variables.
    pub fn into_variables(self) -> hash_map::IntoIter<&'a str, &'a str> {
        self.variables.into_iter()
    }

    /// Splits the scenario into the name and the variables.
    pub fn into_parts(self) -> (Cow<'a, str>, hash_map::IntoIter<&'a str, &'a str>) {
        (self.name, self.variables.into_iter())
    }

    /// Merges several scenarios into one.
    ///
    /// See `Scenario::merge` for more information.
    ///
    /// # Errors
    /// The merge can fail for two reasons:
    /// 1. The iterator `scenarios` was empty – in that case, the
    ///    error `ScenarioError::NoScenarios` is returned.
    /// 2. Strict mode was enabled and two scenarios defined the same
    ///    variable – in that case, the error
    ///    `ScenarioError::StrictMergeFailed` is returned and contains
    ///    the names of the offending scenarios and variable.
    pub fn merge_all<I>(scenarios: I, options: MergeOptions) -> Result<Self>
    where
        I: IntoIterator,
        I::IntoIter: Clone,
        I::Item: Borrow<Self>,
    {
        let mut scenarios = scenarios.into_iter();
        let backup_iter = scenarios.clone();
        let mut accumulator = scenarios
            .next()
            .ok_or(ScenarioError::NoScenarios)?
            .borrow()
            .clone();
        // Go over each scenario `s` and merge it into `accumulator`. Abort on
        // the first error. (`Nothing` is a `()` that allows `collect`ing.)
        let result: Result<Nothing> = scenarios
            .map(|s| accumulator.merge(s.borrow(), options))
            .collect();
        match result {
            Ok(Nothing) => Ok(accumulator),
            Err(mut err) => {
                // If a `StrictMergeFailed` error occurs, the `left` scenario is a
                // merged intermediary. This is useless! Change it to the correct
                // scenario name by searching through `scenarios` once more.
                if let Some(info) = err.strict_merge_failed_info_mut() {
                    info.left = name_of_first_scenario_with_variable(backup_iter, &info.varname)
                        .unwrap();
                }
                Err(err)
            },
        }
    }

    /// Merges another scenario into this one.
    ///
    /// This combines the names and variables of both scenarios. The
    /// names get combined with `options.delimiter` between them.
    /// Variables are combined by adding the `other` `HashMap` into
    /// `self`'s. If both scenarios define the same variable and
    /// `options.strict` is `false`, the value of `other`'s takes
    /// precedence.
    ///
    /// #Errors
    /// If `options.strict` is `true` and both scenarios define the
    /// same variable, `ScenarioError::StrictMergeFailed` is returned.
    pub fn merge(&mut self, other: &Scenario<'a>, options: MergeOptions) -> Result<()> {
        // Turn (&&str, &&str) iterator into (&str, &str) iterator.
        let other_vars = other.variables().map(|(&k, &v)| (k, v));
        // Merge variable definitions first, then the scenario names. If we
        // merged names before the variables, the error message would contain
        // the already-merged name.
        self.merge_vars(other_vars, options.is_strict)
            .context((self.name(), other.name()))?;
        self.merge_name(options.delimiter, &other.name);
        Ok(())
    }

    /// Appends `delimiter` and `other_name` to `self.name`.
    fn merge_name(&mut self, delimiter: &str, other_name: &str) {
        let name = self.name.to_mut();
        name.reserve(delimiter.len() + other_name.len());
        name.push_str(delimiter);
        name.push_str(other_name);
    }

    /// Adds all variable definitions in `to_add` to `self.variables`.
    ///
    /// If `strict` is `true`, this refuses to overwrite existing
    /// variable definitions. In such a case, the offending variable
    /// name is reported in the `Err` variant of the result.
    fn merge_vars<I>(&mut self, to_add: I, strict: bool) -> ::std::result::Result<(), String>
    where
        I: Iterator<Item = (&'a str, &'a str)>,
    {
        if strict {
            for (key, value) in to_add {
                if self.variables.contains_key(key) {
                    return Err(key.to_owned());
                }
                self.variables.insert(key, value);
            }
        } else {
            self.variables.extend(to_add);
        }
        Ok(())
    }
}

impl<'a> Display for Scenario<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Scenario \"{}\"", self.name)
    }
}


/// Wrapper type around customization options to `Scenario::merge()`.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct MergeOptions<'a> {
    /// A string used to join the scenario names together.
    ///
    /// The default is `", "`, a comma followed by a space.
    pub delimiter: &'a str,
    /// Flag that enables strict mode.
    ///
    /// In strict mode, merging fails if two scenarios define the same
    /// variable. By default, strict mode is enabled.
    pub is_strict: bool,
}

impl<'a> MergeOptions<'a> {
    fn new(delimiter: &'a str, is_strict: bool) -> Self {
        MergeOptions {
            delimiter,
            is_strict,
        }
    }
}

impl<'a> Default for MergeOptions<'a> {
    fn default() -> Self {
        MergeOptions {
            delimiter: ", ",
            is_strict: true,
        }
    }
}


/// Opaque elper type for `ScenarioError::StrictMergeFailed`.
#[derive(Debug)]
pub struct StrictMergeFailed {
    varname: String,
    left: String,
    right: String,
}


/// A zero-sized type that implements `FromIterator`.
///
/// This allows us to call `Iterator::collect<Result<_>>` without
/// creating a vector when item type of the iterator is `()`.
struct Nothing;

impl FromIterator<()> for Nothing {
    fn from_iter<T: IntoIterator<Item = ()>>(iter: T) -> Self {
        for _ in iter.into_iter() {}
        Nothing
    }
}


/// Tests if a character is a valid C identifier.
///
/// C identifiers contain only the following characters:
/// * ASCII letters (lowercase or uppercase),
/// * ASCII digits,
/// * the ASCII underscore.
/// Additionally, they must not begin with a digit, and contain at
/// least one character.
fn is_c_identifier(s: &str) -> bool {
    let mut iter = s.as_bytes().iter();
    let first_byte = match iter.next() {
        Some(byte) => byte,
        None => return false,
    };
    match *first_byte {
        b'A'...b'Z' | b'a'...b'z' | b'_' => {},
        _ => return false,
    }
    for byte in s.as_bytes().iter() {
        match *byte {
            b'A'...b'Z' | b'a'...b'z' | b'0'...b'9' | b'_' => {},
            _ => return false,
        }
    }
    true
}


/// Finds a scenario that defines a variable and returns its name.
///
/// This is a helper function to `Scenario::merge_all()`.
fn name_of_first_scenario_with_variable<'a, I>(mut scenarios: I, varname: &str) -> Option<String>
where
    I: Iterator,
    I::Item: Borrow<Scenario<'a>>,
{
    scenarios
        .find(|s| s.borrow().has_variable(varname))
        .map(|s| s.borrow().name().to_owned())
}


quick_error! {
    /// Errors caused during building a scenario.
    #[derive(Debug)]
    pub enum ScenarioError {
        InvalidName(name: String) {
            description("invalid scenario name")
            display(err) -> ("{}: \"{}\"", err.description(), name)
        }
        InvalidVariable(name: String) {
            description("invalid variable name")
            display(err) -> ("{}: \"{}\"", err.description(), name)
        }
        DuplicateVariable(name: String) {
            description("variable already defined")
            display(err) -> ("{}: \"{}\"", err.description(), name)
        }
        NoScenarios {
            description("scenario merge: no scenarios provided")
        }
        StrictMergeFailed(err: StrictMergeFailed) {
            description("conflicting variable definitions")
            display("variable \"{}\" defined both in scenario \"{}\" and in scenario \"{}\"",
                    err.varname, err.left, err.right)
            context(left_right: (&'a str, &'a str), v: String) -> (
                StrictMergeFailed {
                    varname: v,
                    left: left_right.0.to_owned(),
                    right: left_right.1.to_owned(),
                }
            )
        }
    }
}


/// Private helper functions around `ScenarioError`.
impl ScenarioError {
    /// If the error is `StrictMergeFailed`, returns its data.
    fn strict_merge_failed_info(&self) -> Option<&StrictMergeFailed> {
        match *self {
            ScenarioError::StrictMergeFailed(ref err) => Some(err),
            _ => None,
        }
    }

    /// If the error is `StrictMergeFailed`, returns its data.
    fn strict_merge_failed_info_mut(&mut self) -> Option<&mut StrictMergeFailed> {
        match *self {
            ScenarioError::StrictMergeFailed(ref mut err) => Some(err),
            _ => None,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn make_dummy_scenario<'a>(name: &'a str, vars: &[&'a str]) -> Scenario<'a> {
        let mut result = Scenario::new(name).expect(name);
        for var in vars.into_iter().cloned() {
            result.add_variable(var, "").expect(var);
        }
        result
    }


    #[test]
    fn test_is_c_identifier() {
        assert!(is_c_identifier("_"));
        assert!(is_c_identifier("SomeValue"));
        assert!(is_c_identifier("ALL_CAPS_AND_9"));
        assert!(is_c_identifier("l111"));
        assert!(is_c_identifier("__init__"));

        assert!(!is_c_identifier(""));
        assert!(!is_c_identifier("some value"));
        assert!(!is_c_identifier("Mörder"));
        assert!(!is_c_identifier("7"));
        assert!(!is_c_identifier("1a"));
        assert!(!is_c_identifier("🍣"));
    }

    #[test]
    fn test_scenario_new() {
        assert!(Scenario::new("A Name").is_ok());
        assert!(Scenario::new("666").is_ok());
        assert!(Scenario::new("a, b").is_ok());
        assert!(Scenario::new("  ").is_ok());

        assert!(Scenario::new("\0").is_err());
        assert!(Scenario::new("").is_err());
    }

    #[test]
    fn test_scenario_add_variable() {
        let mut s = Scenario::new("name").unwrap();
        // Adding a variable.
        assert!(s.add_variable("key", "value").is_ok());
        // Values may contain spaces.
        assert!(s.add_variable("key2", "a value").is_ok());
        // The same variable must not be added twice.
        assert!(s.add_variable("key", "value").is_err());
        // Variable names must be C identifiers.
        assert!(s.add_variable("a key", "value").is_err());
        assert!(s.add_variable("[key]", "value").is_err());
        // Check that adding occurred.
        assert!(s.has_variable("key"));
        assert!(!s.has_variable("a key"));
    }

    #[test]
    fn test_merge_none() {
        match Scenario::merge_all(&[], MergeOptions::default()).unwrap_err() {
            ScenarioError::NoScenarios => {},
            err => panic!("wrong error: {}", err),
        }
    }

    #[test]
    fn test_merge_one() {
        let expected = make_dummy_scenario("A", &[]);
        // TODO: Improve signature of merge_all to get rid of cloning here.
        let merged = Scenario::merge_all(&[expected.clone()], MergeOptions::default()).unwrap();
        assert_eq!(expected, merged);
    }

    #[test]
    fn test_merge_two() {
        let expected = make_dummy_scenario("A -- B", &["a", "b"]);
        let mut merged = make_dummy_scenario("A", &["a"]);
        let added = make_dummy_scenario("B", &["b"]);
        merged
            .merge(&added, MergeOptions::new(" -- ", true))
            .unwrap();
        assert_eq!(expected, merged);
    }

    #[test]
    fn test_merge_error_two() {
        let expected_message = "variable \"a\" defined both in scenario \"A\" and in scenario \
                                \"B\"";
        let mut merged = make_dummy_scenario("A", &["a"]);
        let added = make_dummy_scenario("B", &["a"]);
        let error = merged
            .merge(&added, MergeOptions::default())
            .unwrap_err();
        assert_eq!(expected_message, error.to_string());
    }

    #[test]
    fn test_merge_error_three() {
        let expected_message = "variable \"a\" defined both in scenario \"A\" and in scenario \
                                \"C\"";
        let scenarios = [
            make_dummy_scenario("A", &["a"]),
            make_dummy_scenario("B", &["b"]),
            make_dummy_scenario("C", &["a"]),
        ];
        let error = Scenario::merge_all(&scenarios, MergeOptions::default()).unwrap_err();
        assert_eq!(expected_message, error.to_string());
    }

    #[test]
    fn test_lax_merge() {
        let expected = make_dummy_scenario("A, B", &["a"]);
        let mut merged = make_dummy_scenario("A", &["a"]);
        let added = make_dummy_scenario("B", &["a"]);
        merged
            .merge(&added, MergeOptions::new(", ", false))
            .unwrap();
        assert_eq!(expected, merged);
    }

    #[test]
    fn test_multi_merge() {
        let expected = make_dummy_scenario("A/B/C", &["a", "aa", "b", "bb", "c", "cc"]);
        let all = [
            make_dummy_scenario("A", &["a", "aa"]),
            make_dummy_scenario("B", &["b", "bb"]),
            make_dummy_scenario("C", &["c", "cc"]),
        ];
        let actual = Scenario::merge_all(&all, MergeOptions::new("/", true)).unwrap();
        assert_eq!(expected, actual);
    }
}
