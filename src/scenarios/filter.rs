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


use glob::{Pattern, MatchOptions};
use failure::Error;

use super::Scenario;


/// Type that allows filtering scenarios based on patterns.
///
/// The name filter has two `Mode`s that it may run in:
///
/// - `Mode::ChooseMatching`: a scenario is allowed to pass if its name
///   matches the pattern given to the filter. If the filter has no
///   pattern, *no* scenarios are excluded.
/// - `Mode::IgnoreMatching`: a scenario is allowed to pass if its name
///   does *not* match the pattern given to the filter. If the filter
///   has no pattern, *all* scenarios are allowed.
///
/// The pattern may be any shell-like glob pattern, in which the
/// patterns `"*"`, `"?"`, `"[...]"` and `"[^...]"` are interpreted
/// specially. (See the `glob` crate for more information.)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameFilter {
    mode: Mode,
    pattern: Option<Pattern>,
}

impl NameFilter {
    /// Creates a new filter running in the given mode.
    pub fn new(mode: Mode) -> Self {
        NameFilter { mode, pattern: None }
    }

    /// Returns `true` if the filter allows this scenario.
    ///
    /// Depending on the filter's `Mode`, the scenario's name must
    /// either match or *not* match the filter's pattern to be allowed.
    pub fn is_allowed(&self, scenario: &Scenario) -> bool {
        let options = MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };
        let matches = self.pattern
            .as_ref()
            .map(|p| p.matches_with(scenario.name(), &options))
            .unwrap_or(false);
        match self.mode {
            Mode::ChooseMatching => matches,
            Mode::IgnoreMatching => !matches,
        }
    }

    /// Returns the filter's `Mode`.
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Sets the filter's `Mode`.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Adds a pattern to this filter.
    ///
    /// In contrast to `set_pattern`, this takes and returns `self`, so
    /// it may be used in a method-call chain.
    pub fn add_pattern(mut self, pattern: &str) -> Result<Self, Error> {
        self.set_pattern(pattern)?;
        Ok(self)
    }

    /// Sets the filter's pattern.
    pub fn set_pattern(&mut self, pattern: &str) -> Result<(), Error> {
        self.pattern = Some(Pattern::new(pattern)?);
        Ok(())
    }

    /// Returns the filter's pattern, if it has one.
    pub fn pattern(&self) -> &Option<Pattern> {
        &self.pattern
    }
}


/// Enum type that specifies the mode in which a `NameFilter` may run.
///
/// The default value is `IgnoreMatching`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    ChooseMatching,
    IgnoreMatching,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::IgnoreMatching
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use scenarios::Scenario;

    #[test]
    fn test_default() {
        let s = Scenario::new("a").unwrap();
        assert!(NameFilter::default().is_allowed(&s));
    }

    #[test]
    fn test_exclusion() {
        let s = Scenario::new("a").unwrap();
        assert!(!NameFilter::new(Mode::ChooseMatching).is_allowed(&s));
    }

    #[test]
    fn test_ignore() {
        let names = ["bark", "berk", "birk", "bork", "burk"];
        let blacklist = NameFilter::default().add_pattern("?i*").unwrap();
        let filtered = names
            .iter()
            .map(|n| Scenario::new(*n).expect(n))
            .filter(|s| blacklist.is_allowed(&s))
            .map(|s| s.name().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(filtered, &["bark", "berk", "bork", "burk"]);
    }

    #[test]
    fn test_choose() {
        let names = ["bark", "berk", "birk", "bork", "burk"];
        let blacklist = NameFilter::new(Mode::ChooseMatching)
            .add_pattern("?[aou]rk")
            .unwrap();
        let filtered = names
            .iter()
            .map(|n| Scenario::new(*n).expect(n))
            .filter(|s| blacklist.is_allowed(&s))
            .map(|s| s.name().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(filtered, &["bark", "bork", "burk"]);
    }
}
