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


//! Contains all scenario-related functionality.
//!
//! This contains, most importantly, the [`Scenario`] type, as well as
//! the [`ScenarioFile`] type, which allows reading scenarios from text
//! files.
//!
//! [`Scenario`]: ./struct.ScenarioFile.html
//! [`ScenarioFile`]: ./struct.ScenarioFile.html


mod filter;
mod inputline;
mod location;
mod scenario;
mod scenario_file;

pub use self::filter::Mode as FilterMode;
pub use self::filter::NameFilter;
pub use self::scenario::MergeOptions;
pub use self::scenario::Scenario;
pub use self::scenario_file::ScenarioFile;
pub use self::scenario_file::ScenariosIter;

pub use self::scenario::MergeError;
pub use self::scenario::ScenarioError;
