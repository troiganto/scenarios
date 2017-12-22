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

/// A consumer of [`Scenario`]s that prints their names to stdout.
///
/// This is a very simple run-time formatter. It takes a template
/// string, replaces all occurrences of `"{}"` in it with a given
/// string, then appends a terminator string to the result. No
/// validation nor sanitation takes place.
///
/// [`Scenario`]: ../scenarios/struct.Scenario.html
#[derive(Debug)]
pub struct Printer<'tpl, 'trm> {
    /// A string in which `PATTERN` is replaced by the scenario name.
    template: &'tpl str,
    /// A string printed after each template.
    terminator: &'trm str,
}

impl<'tpl, 'trm> Printer<'tpl, 'trm> {
    /// Creates a new `Printer` with given template and terminator.
    ///
    /// The template is the string in which all occurrences of
    /// `"{}"` are replaced by the formatted string. The terminator is
    /// the string that is appended to the template afterwards.
    pub fn new(template: &'tpl str, terminator: &'trm str) -> Self {
        Printer { template, terminator }
    }

    /// Creates a new printer that doesn't print anything.
    ///
    /// The returned printer has an empty template and an empty
    /// terminator. This is mostly in cases you want to set these
    /// values yourself.
    pub fn new_null() -> Self {
        Printer::new("", "")
    }

    /// Returns the printer's template string.
    pub fn template(&self) -> &str {
        self.template
    }

    /// Changes the printer's template string.
    pub fn set_template(&mut self, template: &'tpl str) {
        self.template = template;
    }

    /// Returns the printer's terminator string.
    pub fn terminator(&self) -> &str {
        self.terminator
    }

    /// Changes the printer's terminator string.
    pub fn set_terminator(&mut self, terminator: &'trm str) {
        self.terminator = terminator;
    }

    /// Applies the printer to a string.
    ///
    /// This inserts the given string into the template and appends the
    /// terminator to the result.
    ///
    /// # Examples
    ///
    /// ```rust
    /// extern crate scenarios
    /// use scenaros::consumers::Printer;
    /// let p = Printer::new();
    /// assert_eq!(p.format("hello world"), "hello world\n");
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
    /// The default values are empty braces `"{}"` for the `template`
    /// and a newline `"\n"` for the `terminator`.
    fn default() -> Self {
        Printer { template: PATTERN, terminator: "\n" }
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
