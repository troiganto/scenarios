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


//! Provides the method `OsStr::try_to_str()`.


use std::ffi::OsStr;


/// Extension trait on `OsStr`.
pub trait OsStrExt {
    /// Tries to get a `&str` slice if the `OsStr` is valid Unicode.
    ///
    /// This is like `OsStr::to_str`, except it returns a `Result`
    /// instead of an `Option`.
    fn try_to_str(&self) -> Result<&str, NotUtf8>;
}

impl OsStrExt for OsStr {
    fn try_to_str(&self) -> Result<&str, NotUtf8> {
        self.to_str().ok_or_else(|| NotUtf8(self.to_string_lossy().into_owned()))
    }
}


/// The error type of [`OsStrExt`].
///
/// [`OsStrExt`]: ./trait.OsStrExt.html
#[derive(Debug, Fail)]
#[fail(display = "contains invalid UTF-8 character: \"{}\"", _0)]
pub struct NotUtf8(String);
