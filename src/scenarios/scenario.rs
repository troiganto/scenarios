
use std::collections::hash_map::{self, HashMap};
use std::error::Error;
use std::fmt::{self, Display};


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
/// C identifiers. A scenario name must be non-empty.
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
    /// is the empty string or contains a comma.
    pub fn new<S: Into<String>>(name: S) -> Result<Self, ScenarioError> {
        let name = name.into();
        if name.is_empty() {
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
    /// The names get combined with `delimiter` between them. Variables
    /// are combined by adding the `other` `HashMap` into `self`'s.
    /// If both scenarios define the same variable and `strict` is
    /// `false`, the value of `other`'s takes precedence.
    ///
    /// #Errors
    /// If `strict` is `true` and both scenarios define the same
    /// variable, a `ScenarioError::StrictMergeFailed` is returned.
    pub fn merge(&mut self,
                 other: &Scenario,
                 delimiter: &str,
                 strict: bool)
                 -> Result<(), ScenarioError> {
        // Turn (&String, &String) iterator into (String, String) iterator.
        let other_vars = other
            .variables()
            .map(|(k, v)| (k.to_owned(), v.to_owned()));
        // Merge variable definitions. If an error occurs, build a
        // `ScenarioError::StrictMergeFailed` value and return.
        merge_vars(&mut self.variables, other_vars, strict)
            .map_err(|varname| fail_strict_merge(varname, &self.name, &other.name))?;
        // If we merged names before the variables, the error would
        // contain the already-merged name -- thus, we only merge names
        // after merging the variables has succeeded.
        merge_names(&mut self.name, delimiter, &other.name);
        Ok(())
    }
}

impl Display for Scenario {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, r#"Scenario "{}""#, self.name)
    }
}


fn merge_names(left: &mut String, delimiter: &str, right: &str) {
    left.reserve(delimiter.len() + right.len());
    left.push_str(delimiter);
    left.push_str(right);
}

fn merge_vars<I>(map: &mut HashMap<String, String>, to_add: I, strict: bool) -> Result<(), String>
    where I: Iterator<Item = (String, String)>
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


/// Errors caused during building a scenario.
#[derive(Debug)]
pub enum ScenarioError {
    /// The scenario name is invalid.
    InvalidName(String),
    /// The variable name is invalid.
    InvalidVariable(String),
    /// A variable of this name has been added before.
    DuplicateVariable(String),
    /// Two scenarios to be merged define the same variable.
    StrictMergeFailed {
        varname: String,
        left: String,
        right: String,
    },
}

impl Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ScenarioError::*;

        match *self {
            InvalidName(ref name) => write!(f, "{}: {:?}", self.description(), name),
            InvalidVariable(ref name) => write!(f, "{}: {:?}", self.description(), name),
            DuplicateVariable(ref name) => write!(f, "{}: {:?}", self.description(), name),
            StrictMergeFailed {
                ref varname,
                ref left,
                ref right,
            } => {
                write!(f,
                       r#"{}: "{}" defined by scenarios "{}" and "{}""#,
                       self.description(),
                       varname,
                       left,
                       right)
            }
        }
    }
}

impl Error for ScenarioError {
    fn description(&self) -> &str {
        use self::ScenarioError::*;

        match *self {
            InvalidName(_) => "the scenario name is invalid",
            InvalidVariable(_) => "the variable name is invalid",
            DuplicateVariable(_) => "a variable of this name has been added before",
            StrictMergeFailed { .. } => "conflicting variable definitions",
        }
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}

/// Shortens creation of `ScenarioError::StrictMergeFailed` values.
fn fail_strict_merge<S1, S2, S3>(varname: S1, left: S2, right: S3) -> ScenarioError
    where S1: Into<String>,
          S2: Into<String>,
          S3: Into<String>
{
    ScenarioError::StrictMergeFailed {
        varname: varname.into(),
        left: left.into(),
        right: right.into(),
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

        assert!(Scenario::new("a, b").is_ok());
        assert!(Scenario::new("\0").is_ok());
        assert!(Scenario::new("  ").is_ok());
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
