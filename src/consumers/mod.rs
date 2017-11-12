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


mod pool;
mod tokens;
mod printer;
mod children;
mod lifecycle;
mod commandline;


pub use self::printer::Printer;
pub use self::commandline::CommandLine;
pub use self::commandline::Options as CommandLineOptions;
pub use self::lifecycle::LoopDriver;
pub use self::lifecycle::loop_in_process_pool;
pub use self::children::PreparedChild;
pub use self::children::FinishedChild;

pub use self::commandline::VariableNameError;
pub use self::children::Error as ChildError;
