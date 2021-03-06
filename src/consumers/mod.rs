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


//! Provides types that accept scenarios and do something with them.


mod children;
mod commandline;
mod lifecycle;
mod pool;
mod printer;
mod tokens;


pub use self::{
    children::{FinishedChild, PreparedChild, RunningChild},
    commandline::{CommandLine, Options as CommandLineOptions},
    lifecycle::{loop_in_process_pool, LoopDriver},
    pool::{ProcessPool, Select, Slot, WaitForSlot},
    printer::Printer,
    tokens::{PoolToken, TokenStock},
};
