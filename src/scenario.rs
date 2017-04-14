
use std::collections::hash_map::{self, HashMap};

use errors::ScenarioError;


fn is_alnum_identifier(s: &str) -> bool {
    use regex::Regex;
    lazy_static!{
        static ref RE: Regex = Regex::new("^[_[:alpha:]][[:word:]]*$").unwrap();
    }
    RE.is_match(s)
}


/// Named set of environment variable definitions.
///
/// A scenario has a name and a set of environment variable definitions.
/// Each definition has an associated variable name and the corresponding
/// value, both strings. A variable name must follow the rules for regular
/// C identifiers.
///
/// `Scenario`s are created through the `iter_from_file()` function.
#[derive(Clone, Debug)]
pub struct Scenario {
    name: String,
    variables: HashMap<String, String>,
}

impl Scenario {
    /// Creates a new scenario named `name`.
    ///
    /// # Errors
    /// This call fails with `ParseError::InvalidName` if `name`
    /// is the empty string or contains a comma.
    pub fn new<S: Into<String>>(name: S) -> Result<Self, ScenarioError> {
        let name = name.into();
        if name.is_empty() || name.contains(',') {
            return Err(ScenarioError::InvalidName(name));
        }
        Ok(Scenario {
               name: name,
               variables: HashMap::new(),
           })
    }

    /// Adds another variable definition of the current set.
    ///
    /// # Errors
    /// This call fails with `ParseError::InvalidVariable` if `name` is
    /// not a valid variable name (`[A-Za-z_][A-Za-z0-9_]+`). It fails
    /// with `ParseError::DuplicateVariable` if a variable of this name
    /// already has been added to the scenario.
    pub fn add_variable<S1, S2>(&mut self, name: S1, value: S2) -> Result<(), ScenarioError>
        where S1: Into<String>,
              S2: Into<String>
    {
        let name = name.into();
        let value = value.into();
        if self.has_variable(&name) {
            Err(ScenarioError::DuplicateVariable(name))
        } else if !is_alnum_identifier(&name) {
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

    /// Merges another scenario into this one.
    ///
    /// This combines the names and variables of both scenarios.
    /// The names get combined with a ", " (comma+space). Variables are
    /// combined by adding the `other` `HashMap` into `self`'s.
    /// If both scenarios define the same variable, the value of
    /// `other`'s takes precedence.
    pub fn merge(&mut self, other: Scenario) {
        // Merge names.
        self.name.reserve(other.name.len() + 2);
        self.name.push_str(", ");
        self.name.push_str(&other.name);
        // Merge variables.
        for (key, value) in other.into_variables() {
            self.variables.insert(key, value);
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn test_is_alnum_identifier() {
        assert!(is_alnum_identifier("_"));
        assert!(is_alnum_identifier("SomeValue"));
        assert!(is_alnum_identifier("ALL_CAPS_AND_9"));
        assert!(is_alnum_identifier("l111"));
        assert!(is_alnum_identifier("__init__"));


        assert!(!is_alnum_identifier(""));
        assert!(!is_alnum_identifier("some value"));
        assert!(!is_alnum_identifier("Mörder"));
        assert!(!is_alnum_identifier("7"));
        assert!(!is_alnum_identifier("1a"));
        assert!(!is_alnum_identifier("🍣"));
    }


    #[test]
    fn test_scenario_new() {
        assert!(Scenario::new("A Name").is_ok());
        assert!(Scenario::new("666").is_ok());

        assert!(Scenario::new("a, b").is_err());
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
        // Check that adding occurred.
        assert!(s.has_variable("key"));
        assert!(!s.has_variable("a key"));
    }
}
