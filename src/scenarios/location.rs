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


use std::fmt::{self, Display};


/// A type that encodes the location of an error in a file.
///
/// This type is used to provide helpful information in case an error
/// occurs while parsing a scenario file. With it, the exact location
/// can be pin-pointed.
///
/// The type parameter `S` serves to abstract over the name being given
/// as a `&str` or a `String`. The methods `as_ref` and `to_owned` help
/// to convert between these two cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ErrorLocation<S: AsRef<str>> {
    /// The file's name or path.
    ///
    /// If the buffer being read is not a regular file, but instead.
    /// e.g. stdin, any other name can be used as well.
    pub filename: S,
    /// The number of the line in which the error occured.
    ///
    /// Numbering starts with `1`. The value `0` means that the error
    /// is not associated with any line. This can be useful if e.g. an
    /// error happens when opening the file.
    pub lineno: usize,
}

impl<S: AsRef<str>> ErrorLocation<S> {
    /// Creates a new error location without line number information.
    pub fn new(filename: S) -> Self {
        Self { filename, lineno: 0 }
    }

    /// Creates a new error location for a given file and line.
    pub fn with_lineno(filename: S, lineno: usize) -> Self {
        Self { filename, lineno }
    }

    /// Creates a new error location that borrows from `self`.
    pub fn as_ref(&self) -> ErrorLocation<&str> {
        ErrorLocation {
            filename: self.filename.as_ref(),
            lineno: self.lineno,
        }
    }

    /// Creates a new error location that owns its `filename` field.
    ///
    /// Note that this method differs from `ToOwned::to_owned()`. In
    /// particular, the return value does not implement
    /// `Borrow<ErrorLocation<&S>>`. The reason is that our `borrow()`
    /// method does not match the required signature.
    pub fn to_owned(&self) -> ErrorLocation<String> {
        ErrorLocation {
            filename: self.filename.as_ref().to_owned(),
            lineno: self.lineno,
        }
    }
}

impl<S: AsRef<str>> Display for ErrorLocation<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.lineno != 0 {
            write!(f, "in {}:{}", self.filename.as_ref(), self.lineno)
        } else {
            write!(f, "file \"{}\"", self.filename.as_ref())
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let s = ErrorLocation::with_lineno("scenario.ini", 20).to_string();
        assert_eq!(s, "in scenario.ini:20");
    }

    #[test]
    fn test_display_without_lineno() {
        let s = ErrorLocation::new("scenario.ini").to_string();
        assert_eq!(s, "file \"scenario.ini\"");
    }
}
