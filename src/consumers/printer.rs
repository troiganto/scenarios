
use scenarios::Scenario;
use super::Consumer;

pub struct Printer;

impl Consumer for Printer {
    fn consume(&self, _scenario: &Scenario) {}
}
