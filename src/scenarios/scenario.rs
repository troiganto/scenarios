
use std::collections::hash_map::{self, HashMap};
use std::error::Error;
use std::fmt::{self, Display};

use quick_error::ResultExt;

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
pub struct Scenario {
    name: String,
    variables: HashMap<String, String>,
}

impl Scenario {
    /// Creates a new scenario named `name`.
    ///
    /// # Errors
    /// This call fails with `ParseError::InvalidName` if `name`
    /// is the empty string or contains a null byte.
    pub fn new<S: Into<String>>(name: S) -> Result<Self, ScenarioError> {
        let name = name.into();
        if name.is_empty() || name.contains('\0') {
            Err(ScenarioError::InvalidName(name))
        } else {
            Ok(
                Scenario {
                    name,
                    variables: HashMap::new(),
                },
            )
        }
    }

    /// Convenience wrapper around `new()` and `add_variable()`.
    pub fn with_variables<S1, S2, S3, I>(name: S1, variables: I) -> Result<Self, ScenarioError>
    where
        S1: Into<String>,
        S2: Into<String>,
        S3: Into<String>,
        I: IntoIterator<Item = (S2, S3)>,
    {
        let mut s = Scenario::new(name)?;
        for (name, value) in variables {
            s.add_variable(name, value)?;
        }
        Ok(s)
    }

    /// Adds another variable definition of the current set.
    ///
    /// # Errors
    /// This call fails with `ParseError::InvalidVariable` if `name` is
    /// not a valid variable name (`[A-Za-z_][A-Za-z0-9_]+`). It fails
    /// with `ParseError::DuplicateVariable` if a variable of this name
    /// already has been added to the scenario.
    pub fn add_variable<S1, S2>(&mut self, name: S1, value: S2) -> Result<(), ScenarioError>
    where
        S1: Into<String>,
        S2: Into<String>,
    {
        let name = name.into();
        let value = value.into();
        if self.has_variable(&name) {
            Err(ScenarioError::DuplicateVariable(name))
        } else if !is_c_identifier(&name) {
            Err(ScenarioError::InvalidVariable(name))
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
    pub fn get_variable(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(String::as_str)
    }

    /// Returns an iterator over all variable names.
    pub fn variable_names(&self) -> hash_map::Keys<String, String> {
        self.variables.keys()
    }

    /// Returns an iterator over all variables.
    pub fn variables(&self) -> hash_map::Iter<String, String> {
        self.variables.iter()
    }

    /// Returns an iterator over all variables.
    pub fn into_variables(self) -> hash_map::IntoIter<String, String> {
        self.variables.into_iter()
    }

    /// Splits the scenario into the name and the variables.
    pub fn into_parts(self) -> (String, hash_map::IntoIter<String, String>) {
        (self.name, self.variables.into_iter())
    }

    /// Merges another scenario into this one.
    ///
    /// This combines the names and variables of both scenarios.
    /// The names get combined with `delimiter` between them. Variables
    /// are combined by adding the `other` `HashMap` into `self`'s.
    /// If both scenarios define the same variable and `strict` is
    /// `false`, the value of `other`'s takes precedence.
    ///
    /// #Errors
    /// If `strict` is `true` and both scenarios define the same
    /// variable, a `ScenarioError::StrictMergeFailed` is returned.
    pub fn merge(
        &mut self,
        other: &Scenario,
        delimiter: &str,
        strict: bool,
    ) -> Result<(), ScenarioError> {
        // Turn (&String, &String) iterator into (String, String) iterator.
        let other_vars = other
            .variables()
            .map(|(k, v)| (k.to_owned(), v.to_owned()));
        // Merge variable definitions. If an error occurs, build a
        // `ScenarioError::StrictMergeFailed` value and return.
        merge_vars(&mut self.variables, other_vars, strict)
            .context((self.name.as_ref(), other.name.as_ref()))?;
        // If we merged names before the variables, the error would
        // contain the already-merged name -- thus, we only merge names
        // after merging the variables has succeeded.
        merge_names(&mut self.name, delimiter, &other.name);
        Ok(())
    }
}

impl Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Scenario \"{}\"", self.name)
    }
}


fn merge_names(left: &mut String, delimiter: &str, right: &str) {
    left.reserve(delimiter.len() + right.len());
    left.push_str(delimiter);
    left.push_str(right);
}


fn merge_vars<I>(map: &mut HashMap<String, String>, to_add: I, strict: bool) -> Result<(), String>
where
    I: Iterator<Item = (String, String)>,
{
    if strict {
        for (key, value) in to_add {
            if map.contains_key(&key) {
                return Err(key);
            }
            map.insert(key, value);
        }
    } else {
        map.extend(to_add);
    }
    Ok(())
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
        StrictMergeFailed{varname: String, left: String, right: String} {
            description("conflicting variable definitions")
            display(err) -> ("{}: \"{}\" defined by scenarios \"{}\" and \"{}\"",
                             err.description(), varname, left, right)
            context(left_right: (&'a str, &'a str), v: String) -> {
                varname: v,
                left: left_right.0.to_owned(),
                right: left_right.1.to_owned()  // No trailing comma allowed here!
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;


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
}
