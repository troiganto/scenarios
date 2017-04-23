
use scenarios::Scenario;

/// Trait for all consumers of scenarios.
///
/// Consumers are objects that actually do something with scenarios.
/// For the most part, this is either printing their name or setting
/// the environment for a command line with them.
pub trait Consumer {
    /// Do something under the given scenario.
    fn consume(&self, scenario: &Scenario);
}
