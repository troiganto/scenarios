//! Module that provides the function `cartesian::product()`. The
//! name has been chosen entirely for this combination.


/// Iterates over the Cartesian product of a list of containers.
///
/// This essentially does the same as the macro `itertools::iproduct`,
/// but the number of arguments may be decided at run-time.
/// In return, this function requires that all passed iterators
/// yield items of the same type, whereas the iterators passed to
/// `itertools::iproduct` may be heterogenous.
///
/// The trait bounds are as follows: The argument to this function must
/// be an immutable slice of containers `C` with items `T`. *Immutable
/// references* to these containers must be convertible to iterators
/// (over `&T`). This is necessary because `product()` needs to iterate
/// over these containers multiple times, so calling `into_iter` must
/// not consume the passed containers. Finally, the lifetime `'a` ties
/// all the used references to the sclice originally passed to
/// `product()`.
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
/// as a whole is an empty iterator, too.
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
    let iterators = collections
        .iter()
        .map(IntoIterator::into_iter)
        .collect();
    let next_item = vec![None; collections.len()];
    let mut product = Product {
        collections,
        iterators,
        next_item,
    };
    // Fill `next_item`, to finish initialization.
    product.fill_up_next_item();
    product
}


/// Iterator over the Cartesian product of some sub-iterators.
pub struct Product<'a, C: 'a, T: 'a>
where
    &'a C: IntoIterator<Item = &'a T>,
{
    collections: &'a [C],
    iterators: Vec<<&'a C as IntoIterator>::IntoIter>,
    next_item: Vec<Option<&'a T>>,
}

impl<'a, C, T> Iterator for Product<'a, C, T>
where
    &'a C: IntoIterator<Item = &'a T>,
{
    type Item = Vec<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        // At this point, the last called method was `fill_up_next_item()`.
        // That means that `next_item` should only contain `Some`s and
        // `collect()` should hence return `Some(Vec(&T))`.
        // If `collect()` returns `None`, there are two cases:
        // 1. `next_item` contains only `None`s at this point -- we
        //    have finished iteration and are exhausted.
        // 2. `next_item` contains some `Some`s and some `None`s -- at
        //    least one of the underlying collections is empty and we
        //    should yield not one item.
        // If there are *no* underlying collections, `collect()` would
        // return `Some(empty_vec)` indefinitely. (The *nullary* case)
        // In this case, we manually exhaust the iterator a bit further
        // down.
        let result = self.next_item
            .iter()
            .cloned()
            .collect::<Option<Self::Item>>();
        if result.is_some() {
            // We are not exhausted yet, prepare the next iteration.
            if self.is_nullary() {
                // See above for why we do this.
                self.exhaust();
            } else {
                // `advance_iterators()` leaves a string of `None`s on
                // the right side of `next_item`. `fill_up_next_item()`
                // replaces them with `Some`s. `is_exhausted` keeps us
                // from cycling forever.
                self.advance_iterators();
                if !self.is_exhausted() {
                    self.fill_up_next_item();
                }
            }
        }
        // And now, we have two cases -- either the iterator is exhausted, or
        // the last method called was `fill_up_next_item()`. Thus, we have
        // re-established the
        result
    }
}

impl<'a, C, T> Product<'a, C, T>
where
    &'a C: IntoIterator<Item = &'a T>,
{
    /// Fills the `None`s in `next_item` from left to right.
    ///
    /// This function assumes and guarantees that `next_item` is
    /// *partitioned*, i.e. that consists of `Some`s on the left and
    /// `None`s on the right.
    ///
    /// None of the sub-iterators should be exhausted at this point. If
    /// one of them is, this function aborts immediately to keep
    /// `next_item` partitioned.
    fn fill_up_next_item(&mut self) {
        let lockstep = self.iterators
            .iter_mut()
            .zip(self.next_item.iter_mut())
            .skip_while(|&(_, ref e)| e.is_some());
        for (iterator, element) in lockstep {
            *element = iterator.next();
            if element.is_none() {
                return;
            }
        }
    }

    /// Advances and cycles the sub-iterators.
    ///
    /// This function assumes that `next_item` is filled with `Some`s.
    /// It goes over them in from right to left and checks for each if
    /// it is exhausted. Exhausted iterators are noted with a `None` in
    /// `next_item` and started anew. The first iterator that isn't
    /// exhausted gets to put its next item into `next_item` and aborts
    /// the function.
    ///
    /// As a result, `next_item` is in a partitioned state after this
    /// call, i.e. all the elements to the left are `Some`s and all the
    /// elements to the right are `None`s. That means that after it,
    /// `fill_up_next_item` should be called to replace the `None`s
    /// with `Some`s.
    ///
    /// The only exception is when _all_ elements of `next_item` are
    /// `None`. This implies that all sub-iterators had to be restarted
    /// at once. This, in turn, means that we have gone through all
    /// combinations of the input and the iterator is, in fact, done.
    /// The implementation should check for this case before calling
    /// `fill_up_next_item`.
    fn advance_iterators(&mut self) {
        let lockstep = self.collections
            .iter()
            .zip(self.iterators.iter_mut())
            .zip(self.next_item.iter_mut())
            .rev();
        for ((collection, iterator), element) in lockstep {
            *element = iterator.next();
            match *element {
                Some(_) => break,
                None => *iterator = collection.into_iter(),
            }
        }
    }

    /// Checks if we have no collections at all.
    ///
    /// In the nullary case, we have to manually set `next_item` to a
    /// value that signals exhaustedness. Otherwise,
    /// `advance_iterators()` will do this for free.
    fn is_nullary(&self) -> bool {
        self.collections.is_empty()
    }

    /// Checks if the iterator is exhausted.
    ///
    /// The iterator is exhausted if `next_item` contains at least one
    /// `None`. We have to take care of the nullary-but-not-exhausted
    /// case, which is an empty `next_item`.
    fn is_exhausted(&self) -> bool {
        match self.next_item.first() {
            Some(&None) => true,
            _ => false,
        }
    }

    /// Manually exhausts this iterator.
    fn exhaust(&mut self) {
        self.next_item = vec![None];
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
