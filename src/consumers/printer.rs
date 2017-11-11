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


use std::io::{self, Write};

use scenarios::Scenario;

/// The string pattern that gets replaced in `Printer::template`.
const PATTERN: &'static str = "{}";

/// A `Consumer` of `Scenario`s that prints their name to `stdout`.
#[derive(Debug)]
pub struct Printer<'template, 'terminator> {
    /// A string in which `PATTERN` is replaced by the scenario name.
    template: &'template str,
    /// A string printed after each template.
    terminator: &'terminator str,
}

impl<'template, 'terminator> Printer<'template, 'terminator> {
    /// Creates a new `Printer` with given template and terminator.
    ///
    /// The template is the string in which all occurrences of
    /// `PATTERN` are replaced by the formatted string. To the result
    /// of this, the terminator is appended.
    pub fn new(template: &'template str, terminator: &'terminator str) -> Self {
        Printer {
            template,
            terminator,
        }
    }

    /// Creates a new `Printer` that doesn't print anything.
    ///
    /// The returned printer has an empty template and an empty
    /// terminator. This is mostly in cases you want to set these
    /// values yourself.
    pub fn new_null() -> Self {
        Printer::new("", "")
    }

    pub fn template(&self) -> &str {
        self.template
    }

    pub fn set_template(&mut self, template: &'template str) {
        self.template = template;
    }

    pub fn terminator(&self) -> &str {
        self.terminator
    }

    pub fn set_terminator(&mut self, terminator: &'terminator str) {
        self.terminator = terminator;
    }

    /// Applies the printer to a string.
    ///
    /// This inserts `s` in `template` and appends `terminator` to the
    /// result.
    ///
    /// # Examples
    ///
    /// ```rust
    /// extern crate scenarios
    /// use scenaros::consumers::Printer;
    /// let p = Printer::new()
    /// assert_eq!(p.format("hello world"), "hello world\n".to_owned());
    /// ```
    pub fn format(&self, s: &str) -> String {
        let mut result = self.template.replace(PATTERN, s);
        result.push_str(self.terminator);
        result
    }

    /// Formats the scenario's name and prints it to `stdout`.
    pub fn print_scenario(&self, scenario: &Scenario) {
        let s = self.format(scenario.name());
        io::stdout().write_all(s.as_bytes()).unwrap();
    }
}

impl<'a, 'b> Default for Printer<'a, 'b> {
    /// Creates a new `Printer` with default values.
    ///
    /// The default values are `PATTERN` (i.e. `"{}"`) for `template`
    /// and a newline (i.e. `"\n"`) for `terminator`.
    fn default() -> Self {
        Printer {
            template: PATTERN,
            terminator: "\n",
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        assert_eq!(Printer::default().format("test"), "test\n");
    }

    #[test]
    fn test_null() {
        assert_eq!(Printer::new_null().format("test"), "");
    }

    #[test]
    fn test_complicated_pattern() {
        assert_eq!(
            Printer::new("{} middle {}", "").format("edge"),
            "edge middle edge"
        );
    }

    #[test]
    fn test_broken_pattern() {
        assert_eq!(
            Printer::new("{{}} {no} {", "}").format("yes"),
            "{yes} {no} {}"
        );
    }
}
