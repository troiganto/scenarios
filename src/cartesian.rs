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


//! Provides the function `cartesian::product()`.
//!
//! The name has been chosen entirely for this combination.


/// Iterates over the Cartesian product of a list of containers.
///
/// This essentially does the same as the macro
/// `itertools::iproduct!()`, but the number of arguments may be
/// decided at run-time. In return, this function requires that all
/// passed iterators yield items of the same type, whereas the
/// iterators passed to `itertools::iproduct!()` may be heterogenous.
/// Furthermore, the freedom of choosing the number of arguments at
/// run-time means that the product iterator iterates over vectors
/// instead of slices. This requires a heap allocation for every item.
///
/// The argument to this function is a slice of containers `C` with
/// items `T`. *Immutable references* to these containers must be
/// convertible to iterators over `&T`. This is necessary because we
/// need to pass over each container multiple times.
///
/// # Example
///
/// ```rust
/// extern crate scenarios;
///
/// use scenarios::cartesian;
///
/// let slices = [[1, 2], [11, 22]];
/// let combinations = cartesian::product(&slices);
/// assert_eq!(combinations.next(), Some(vec![1, 11]));
/// assert_eq!(combinations.next(), Some(vec![1, 22]));
/// assert_eq!(combinations.next(), Some(vec![2, 11]));
/// assert_eq!(combinations.next(), Some(vec![2, 22]));
/// assert_eq!(combinations.next(), None);
/// ```
///
/// Note that if any one of the passed containers is empty, the product
/// as a whole is empty, too.
///
/// ```rust
/// extern crate scenarios;
///
/// use scenarios::cartesian;
///
/// let vectors = [vec![1, 2], vec![11, 22], vec![]];
/// let combinations = cartesian::product(&slices);
/// assert_eq!(combinations.next(), None);
/// ```
pub fn product<'a, C: 'a, T: 'a>(collections: &'a [C]) -> Product<'a, C, T>
where
    &'a C: IntoIterator<Item = &'a T>,
{
    // We start with fresh iterators and a `next_item` full of `None`s.
    let mut iterators = collections
        .iter()
        .map(<&C>::into_iter)
        .collect::<Vec<_>>();
    let next_item = iterators.iter_mut().map(Iterator::next).collect();
    Product { collections, iterators, next_item }
}


/// Iterator returned by [`product()`].
///
/// [`product()`]: ./fn.product.html
pub struct Product<'a, C: 'a, T: 'a>
where
    &'a C: IntoIterator<Item = &'a T>,
{
    /// The underlying collections that we iterate over.
    collections: &'a [C],
    /// Our own set of sub-iterators, taken from `collections`.
    iterators: Vec<<&'a C as IntoIterator>::IntoIter>,
    /// The next item to yield.
    next_item: Option<Vec<&'a T>>,
}

impl<'a, C, T> Iterator for Product<'a, C, T>
where
    &'a C: IntoIterator<Item = &'a T>,
{
    type Item = Vec<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.next_item.clone();
        self.advance();
        result
    }
}

impl<'a, C, T> Product<'a, C, T>
where
    &'a C: IntoIterator<Item = &'a T>,
{
    /// Advances the iterators and updates `self.next_item`.
    ///
    /// This loop works like incrementing a number digit by digit. We
    /// go over each iterator and its corresponding "digit" in
    /// `next_item` in lockstep, starting at the back.
    ///
    /// If we can advance the iterator, we update the "digit" and are
    /// done. If the iterator is exhausted, we have to go from "9" to
    /// "10": we restart the iterator, grab the first element, and move
    /// on to the next digit.
    ///
    /// The `break` expressions are to be understood literally: our
    /// scheme can break in two ways.
    /// 1. The very first iterator (`i==0`) is exhausted.
    /// 2. A freshly restarted iterator is empty. (should never happen!)
    /// In both cases, we want to exhaust `self` immediately. We do so
    /// by breaking out of the loop, falling through to the very last
    /// line, and manually set `self.next_item` to `None`.
    ///
    /// Note that there is a so-called nullary case, when
    /// `cartesian::product()` is called with an empty slice. While
    /// this use-case is debatable, the mathematically correct way to
    /// deal with it is to yield some empty vector once and then
    /// nothing.
    ///
    /// Luckily, we already handle this correctly! Because of the way
    /// `Iterator::collect()` works when collecting into an
    /// `Option<Vec<_>>`, `next_item` is initialized to some empty
    /// vector, so this will be the first thing we yield. Then, when
    /// `self.advance()` is called, we fall through the `while` loop and
    /// immediately exhaust this iterator, yielding nothing more.
    fn advance(&mut self) {
        if let Some(ref mut next_item) = self.next_item {
            let mut i = self.iterators.len();
            while i > 0 {
                i -= 1;
                // Grab the next item from the current sub-iterator.
                if let Some(elt) = self.iterators[i].next() {
                    next_item[i] = elt;
                    // If that works, we're done!
                    return;
                } else if i == 0 {
                    // Last sub-iterator is exhausted, so we're
                    // exhausted, too.
                    break;
                }
                // The current sub-terator is empty, start anew.
                self.iterators[i] = self.collections[i].into_iter();
                if let Some(elt) = self.iterators[i].next() {
                    next_item[i] = elt;
                    // Roll over to the next sub-iterator.
                } else {
                    // Should never happen: The freshly restarted
                    // sub-iterator is already empty.
                    break;
                }
            }
        }
        // Exhaust this iterator if the above loop `break`s.
        self.next_item = None;
    }
}


#[cfg(test)]
mod tests {
    mod lengths {
        use cartesian;

        /// Asserts that the `len(V1×V2×...VN) ==
        /// len(V1)×len(V2)×...len(VN)`.
        fn assert_length<T>(vectors: &Vec<Vec<T>>) {
            let expected_len: usize = vectors.iter().map(Vec::len).product();
            let actual_len: usize = cartesian::product(vectors)
                .collect::<Vec<Vec<&T>>>()
                .len();
            assert_eq!(expected_len, actual_len);
        }

        #[test]
        fn test_length() {
            let vectors = vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2], vec![3, 3, 3, 3]];
            assert_length(&vectors);
        }

        #[test]
        fn test_unequal_length() {
            let vectors = vec![vec![1, 1], vec![2, 2, 2, 2], vec![3]];
            assert_length(&vectors);
        }

        #[test]
        fn test_empty_vector() {
            let one_is_empty = [vec![0; 3], vec![0; 3], vec![0; 0]];
            let empty_product: Vec<_> = cartesian::product(&one_is_empty).collect();
            assert_eq!(empty_product.len(), 0);
        }

        #[test]
        fn test_nullary_product() {
            let empty: [[u32; 1]; 0] = [];
            let mut nullary_product = cartesian::product(&empty);
            assert_eq!(nullary_product.next(), Some(Vec::new()));
            assert_eq!(nullary_product.next(), None);
        }
    }


    mod types {
        use cartesian;

        #[test]
        fn test_i32() {
            let numbers = [[0, 16, 32, 48], [0, 4, 8, 12], [0, 1, 2, 3]];
            let expected: Vec<u32> = (0..64).collect();
            let actual: Vec<u32> = cartesian::product(&numbers)
                .map(Vec::into_iter)
                .map(Iterator::sum)
                .collect();
            assert_eq!(expected, actual);
        }

        #[test]
        fn test_string() {
            use std::iter::FromIterator;

            let letters = [
                ["A".to_string(), "B".to_string()],
                ["a".to_string(), "b".to_string()],
            ];
            let expected = vec![
                "Aa".to_string(),
                "Ab".to_string(),
                "Ba".to_string(),
                "Bb".to_string(),
            ];
            let actual: Vec<String> = cartesian::product(&letters)
                .map(|combo| combo.into_iter().map(String::as_str))
                .map(String::from_iter)
                .collect();
            assert_eq!(expected, actual);
        }

        #[test]
        fn test_slices() {
            let bits: [[u8; 2]; 4] = [[0, 8], [0, 4], [0, 2], [0, 1]];
            let expected: Vec<u8> = (0..16).collect();
            let actual: Vec<u8> = cartesian::product(&bits)
                .map(Vec::into_iter)
                .map(Iterator::sum)
                .collect();
            assert_eq!(expected, actual);
        }
    }
}
