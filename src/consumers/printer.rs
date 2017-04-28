
use scenarios::Scenario;
use super::Consumer;

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
    /// Creates a new `Printer` with default values.
    ///
    /// The default values are `PATTERN` (i.e. `"{}"`) for `template`
    /// and a newline (i.e. `"\n"`) for `terminator`.
    pub fn new() -> Self {
        Printer {
            template: PATTERN,
            terminator: "\n",
        }
    }

    pub fn template(&self) -> &str {
        self.template
    }

    pub fn set_template<'s: 'template>(&mut self, template: &'s str) {
        self.template = template;
    }

    pub fn with_template<'s: 'template>(mut self, template: &'s str) -> Self {
        self.set_template(template);
        self
    }

    pub fn terminator(&self) -> &str {
        self.terminator
    }

    pub fn set_terminator<'s: 'terminator>(&mut self, terminator: &'s str) {
        self.terminator = terminator;
    }

    pub fn with_terminator<'s: 'terminator>(mut self, terminator: &'s str) -> Self {
        self.set_terminator(terminator);
        self
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
}

impl<'a, 'b> Default for Printer<'a, 'b> {
    fn default() -> Self {
        Printer::new()
    }
}

impl<'a, 'b> Consumer for Printer<'a, 'b> {
    /// Prints formatted scenario names to `stdout`.
    fn consume(&self, scenario: &Scenario) {
        print!("{}", self.format(scenario.name()));
    }
}
