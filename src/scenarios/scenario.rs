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


use std::fmt::{self, Display};
use std::borrow::{Borrow, Cow};
use std::collections::hash_map::{self, HashMap};


/// Named set of environment variable definitions.
///
/// A scenario has a name and a set of environment variable
/// definitions. Each definition consists of a variable name and the
/// corresponding variable value, both strings. A variable name must
/// follow the rules for regular C identifiers. A scenario name must be
/// non-empty and not contain any null byte.
///
/// Note: The rules for regular C identifiers are as follows: The name
/// must contain only the 26 Latin characters (upper- or lowercase),
/// the underscore, and the ten digits of the ASCII character set. The
/// first character must not be a digit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario<'a> {
    name: Cow<'a, str>,
    variables: HashMap<&'a str, &'a str>,
}

impl<'a> Scenario<'a> {
    /// Creates a new scenario named `name`.
    ///
    /// # Errors
    /// This call fails with [`InvalidName`] if `name` is the empty
    /// string or contains a null byte.
    ///
    /// [`InvalidName`]: ./enum.ScenarioError.html#variant.InvalidName
    pub fn new<S: Into<Cow<'a, str>>>(name: S) -> Result<Self, ScenarioError> {
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
    /// This call fails with [`InvalidVariable`] if `name` is not a
    /// valid variable name. It fails with [`DuplicateVariable`] if a
    /// variable of this name already has been added to the scenario.
    ///
    /// [`InvalidVariable`]:
    /// ./enum.ScenarioError.html#variant.InvalidVariable
    /// [`DuplicateVariable`]:
    /// ./enum.ScenarioError.html#variant.DuplicateVariable
    pub fn add_variable(&mut self, name: &'a str, value: &'a str) -> Result<(), ScenarioError> {
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
    /// See [`merge()`] for more information.
    ///
    /// # Errors
    /// The merge can fail if strict mode was enabled and two scenarios
    /// define the same variable.
    ///
    /// # Panics
    /// This function panics if `scenarios` turns into an empty
    /// iterator.
    ///
    /// [`merge()`]: #method.merge
    pub fn merge_all<I>(scenarios: I, opts: MergeOptions) -> Result<Self, MergeError>
    where
        I: IntoIterator,
        I::IntoIter: Clone,
        I::Item: Borrow<Self>,
    {
        let mut scenarios = scenarios.into_iter();
        let backup_iter = scenarios.clone();
        let mut accumulator = scenarios
            .next()
            .expect("no scenarios to merge")
            .borrow()
            .clone();
        // Go over each scenario `s` and merge it into `accumulator`. Abort on
        // the first error.
        let result: Result<(), MergeError> = scenarios
            .map(|s| accumulator.merge(s.borrow(), opts))
            .collect();
        match result {
            Ok(()) => Ok(accumulator),
            Err(mut err) => {
                // If a `StrictMergeFailed` error occurs, the `left` scenario is a
                // merged intermediary. This is useless! Change it to the correct
                // scenario name by searching through `scenarios` once more.
                err.left = name_of_first_scenario_with_variable(backup_iter, &err.varname).unwrap();
                Err(err)
            },
        }
    }

    /// Merges another scenario into this one.
    ///
    /// This combines the names and variables of both scenarios. The
    /// names get combined with [`opts.delimiter`] between them.
    /// Variables are combined by adding definitions from `other` to
    /// `self`. If both scenarios define the same variable and
    /// [`opts.is_strict`] is `false`, the value of `other`'s
    /// variable takes precedence.
    ///
    /// # Errors
    /// If [`opts.is_strict`] is `true` and both scenarios define the
    /// same variable, [`MergeError`] is returned.
    ///
    /// [`opts.delimiter`]:
    /// ./struct.MergeOptions.html#structfield.delimiter
    /// [`opts.is_strict`]:
    /// ./struct.MergeOptions.html#structfield.is_strict
    /// [`MergeError`]: ./struct.MergeError.html
    pub fn merge(&mut self, other: &Scenario<'a>, opts: MergeOptions) -> Result<(), MergeError> {
        // Turn (&&str, &&str) iterator into (&str, &str) iterator.
        let other_vars = other.variables().map(|(&k, &v)| (k, v));
        // Merge variable definitions first, then the scenario names. If we
        // merged names before the variables, the error message would contain
        // the already-merged name.
        self.merge_vars(other_vars, opts.is_strict)
            .map_err(|var| MergeError::new(var, self.name(), other.name()))?;
        self.merge_name(opts.delimiter, &other.name);
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


/// Wrapper type around customization options to [`Scenario::merge()`].
///
/// [`Scenario::merge()`]: ./struct.Scenario.html#method.merge
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
        MergeOptions { delimiter, is_strict }
    }
}

impl<'a> Default for MergeOptions<'a> {
    fn default() -> Self {
        MergeOptions { delimiter: ", ", is_strict: true }
    }
}


/// Tests if a character is a valid C identifier.
///
/// C identifiers contain only the following characters:
///
/// - ASCII letters (lowercase or uppercase),
/// - ASCII digits,
/// - the ASCII underscore.
///
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
///
/// [`Scenario::merge_all()`]: ./struct.Scenario.html#method.merge_all
fn name_of_first_scenario_with_variable<'a, I>(mut scenarios: I, varname: &str) -> Option<String>
where
    I: Iterator,
    I::Item: Borrow<Scenario<'a>>,
{
    scenarios
        .find(|s| s.borrow().has_variable(varname))
        .map(|s| s.borrow().name().to_owned())
}


/// Errors that may occur when building a [`Scenario`].
///
/// [`Scenario`]: ./struct.Scenario.html
#[derive(Debug, Fail)]
pub enum ScenarioError {
    /// The scenario name is illegal.
    #[fail(display = "invalid scenario name: \"{}\"", _0)]
    InvalidName(String),
    /// The variable name is illegal.
    #[fail(display = "invalid variable name: \"{}\"", _0)]
    InvalidVariable(String),
    /// The variable name has already been used..
    #[fail(display = "variable already defined: \"{}\"", _0)]
    DuplicateVariable(String),
}


/// Errors caused by conflicting variables during merging of scenarios.
///
/// This error may be returned by [`Scenario::merge()`] and
/// [`Scenario::merge_all()`].
///
/// [`Scenario::merge()`]: ./struct.Scenario.html#method.merge
/// [`Scenario::merge_all()`]: ./struct.Scenario.html#method.merge_all
#[derive(Debug, Fail)]
#[fail(display = "variable \"{}\" defined both in scenario \"{}\" and in scenario \"{}\"",
       varname, left, right)]
pub struct MergeError {
    varname: String,
    left: String,
    right: String,
}

impl MergeError {
    fn new<V, L, R>(varname: V, left: L, right: R) -> Self
    where
        V: Into<String>,
        L: Into<String>,
        R: Into<String>,
    {
        MergeError {
            varname: varname.into(),
            left: left.into(),
            right: right.into(),
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
    #[should_panic]
    fn test_merge_none_panics() {
        let _ = Scenario::merge_all(&[], MergeOptions::default());
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
