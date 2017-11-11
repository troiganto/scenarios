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


mod inputline;
mod location;
mod scenario;
mod scenario_file;

pub use self::scenario::Scenario;
pub use self::scenario::MergeOptions;
pub use self::scenario_file::ScenarioFile;
pub use self::scenario_file::ScenariosIter;

pub use self::inputline::SyntaxError;
pub use self::scenario::ScenarioError;
pub use self::scenario_file::ParseError;

pub use self::scenario::Result;
