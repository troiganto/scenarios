
use scenarios::Scenario;
use super::Consumer;

const PATTERN: &'static str = "{}";

#[derive(Debug)]
pub struct Printer<'template, 'terminator> {
    template: &'template str,
    terminator: &'terminator str,
}

impl<'template, 'terminator> Printer<'template, 'terminator> {
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

    pub fn format(&self, s: &str) -> String {
        let mut result = self.template.replace(PATTERN, s);
        result.push_str(self.terminator);
        result
    }
}

impl<'template, 'terminator> Default for Printer<'template, 'terminator> {
    fn default() -> Self {
        Printer::new()
    }
}

impl<'template, 'terminator> Consumer for Printer<'template, 'terminator> {
    fn consume(&self, scenario: &Scenario) {
        print!("{}", self.format(scenario.name()));
    }
}
