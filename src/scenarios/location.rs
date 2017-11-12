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


use std::borrow::Borrow;
use std::fmt::{self, Display};


/// A type that encodes the location of an error in a file.
///
/// This type is used to provide helpful information in case an error
/// occurs while parsing a scenario file. With it, the exact location
/// can be pin-pointed.
///
/// The type parameter `S` serves to abstract over the name being given
/// as a `&str` or a `String`. The methods `as_ref` and `to_owned` help
/// to convert between these two cases. (Note that these are inherent
/// methods. This type implements neither `AsRef` nor `Borrow` nor
/// `ToOwned` beyond `std`'s blanket implementations.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ErrorLocation<S> {
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

impl<S> ErrorLocation<S> {
    /// Creates a new error location without line number information.
    pub fn new(filename: S) -> Self {
        Self { filename, lineno: 0 }
    }

    /// Creates a new error location for a given file and line.
    pub fn with_lineno(filename: S, lineno: usize) -> Self {
        Self { filename, lineno }
    }

    /// Creates a new error location that borrows from `self`.
    ///
    /// Note that the signature differs from `Borrow::borrow()`. This
    /// does not return a reference, but instead a value that contains
    /// a reference.
    pub fn borrow<Borrowed>(&self) -> ErrorLocation<&Borrowed>
    where
        S: Borrow<Borrowed>,
        Borrowed: ?Sized,
    {
        ErrorLocation {
            filename: self.filename.borrow(),
            lineno: self.lineno,
        }
    }
}

impl<'a, S> ErrorLocation<&'a S>
where
    S: ToOwned + ?Sized,
{
    /// Creates a new error location that owns its `filename` field.
    ///
    /// Note that this method differs from `ToOwned::to_owned()`. In
    /// particular, the return value does not implement
    /// `Borrow<ErrorLocation<&S>>`. The reason is that our `borrow()`
    /// method does not match the required signature.
    pub fn to_owned(&self) -> ErrorLocation<S::Owned> {
        ErrorLocation {
            filename: self.filename.to_owned(),
            lineno: self.lineno,
        }
    }
}

impl<S: Display> Display for ErrorLocation<S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.lineno != 0 {
            write!(f, "{}:{}", self.filename, self.lineno)
        } else {
            write!(f, "{}", self.filename)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let s = ErrorLocation::with_lineno("scenario.ini", 20).to_string();
        assert_eq!(s, "scenario.ini:20");
    }

    #[test]
    fn test_display_without_lineno() {
        let s = ErrorLocation::new("scenario.ini").to_string();
        assert_eq!(s, "scenario.ini");
    }

    #[test]
    fn test_owned_to_borrowed() {
        let owned = ErrorLocation::new(String::from("-"));
        let _: ErrorLocation<&str> = owned.borrow();
        let _: ErrorLocation<String> = owned;
    }

    #[test]
    fn test_borrowed_to_owned() {
        let borrowed = ErrorLocation::new("-");
        let _: ErrorLocation<String> = borrowed.to_owned();
        let _: ErrorLocation<&str> = borrowed;
    }

    #[test]
    fn test_borrowed_to_borrowed() {
        let borrowed = ErrorLocation::new("-");
        let _: ErrorLocation<&str> = borrowed.borrow();
        let _: ErrorLocation<&str> = borrowed;
    }

    #[test]
    fn test_owned_to_owned() {
        let owned = ErrorLocation::new(String::from("-"));
        let _: ErrorLocation<String> = owned.to_owned();
        let _: ErrorLocation<String> = owned;
    }

    #[test]
    fn test_copy_semantics() {
        let original = ErrorLocation::new("-");
        let copy = original;
        let _ = original;
        let _ = copy;
    }

    #[test]
    fn test_ord() {
        let expected = vec![
            ErrorLocation::with_lineno("a.ini", 9),
            ErrorLocation::with_lineno("a.ini", 12),
            ErrorLocation::with_lineno("b.ini", 1),
        ];
        let mut actual = vec![
            ErrorLocation::with_lineno("b.ini", 1),
            ErrorLocation::with_lineno("a.ini", 12),
            ErrorLocation::with_lineno("a.ini", 9),
        ];
        actual.sort_unstable();
        assert_eq!(expected, actual);
    }
}
