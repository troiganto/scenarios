
use scenarios::Scenario;
use super::Consumer;

pub struct CommandLine;

impl Consumer for CommandLine {
    fn consume(&self, _scenario: &Scenario) {}
}
