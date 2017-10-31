//! Module that provides the function `cartesian::product()`. The
//! name has been chosen entirely for this combination.


/// Iterates over Cartesian product of a list of containers.
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
/// all the used references to the reference originally passed to
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
    // Create an unitialized object.
    // we have to fill `iterators` and `next_item`.
    let len = collections.len();
    let mut product = Product {
        collections: collections,
        iterators: Vec::with_capacity(len),
        next_item: ::std::iter::repeat(None).take(len).collect(),
    };

    // Create one brand-new iterator per collection.
    for collection in product.collections {
        product.iterators.push(collection.into_iter());
    }

    // Fill `next_item`, which is full of Nones until now.
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
        if self.next_item.iter().any(Option::is_none) {
            // If any element is None, it means at least one of the
            // sub-iterators is exhausted and this iterator is
            // exhausted as a whole. We are done then.
            None
        } else {
            // None of the elements is `None`, this means we can simply
            // unwrap them.
            let next_item = self.next_item
                .iter()
                .cloned()
                .map(Option::unwrap)
                .collect();
            self.advance_iterators();
            Some(next_item)
        }
    }
}

impl<'a, C, T> Product<'a, C, T>
where
    &'a C: IntoIterator<Item = &'a T>,
{
    fn fill_up_next_item(&mut self) {
        for (iterator, element) in
            self.iterators
                .iter_mut()
                .zip(self.next_item.iter_mut())
                .skip_while(|&(_, ref e)| e.is_some()) {
            *element = iterator.next();
            // If any sub-iterator is exhausted at this point, it means
            // this iterator as a whole is exhausted.
            if element.is_none() {
                return;
            }
        }
    }

    fn advance_iterators(&mut self) {
        // Now that we've extracted the `next_item`, we need to
        // advance the iterator. We call `next()` on the
        // sub-iterators, starting at the back. If they return
        // `None`, we replace them, but keep the `None`. We replace
        // it later with `fill_up_next_item()`.
        for (collection, iterator, element) in
            self.collections
                .iter()
                .zip(self.iterators.iter_mut())
                .zip(self.next_item.iter_mut())
                .map(|((c, i), e)| (c, i, e))
                .rev() {
            *element = iterator.next();
            match *element {
                Some(_) => break,
                None => *iterator = collection.into_iter(),
            }
        }
        // Here, `next_item` consists of Somes on the left and Nones on
        // the right. If the Nones reach all the way to the right, all
        // sub-iterators at once were exhausted. This means, we've got
        // all combinations and are done. Otherwise, we still got ways
        // to go and have to fill up all Nones from newly-created
        // iterators.
        if self.next_item
               .first()
               .expect("next item is never empty")
               .is_some() {
            self.fill_up_next_item();
        }
    }
}


#[cfg(test)]
mod tests {
    use super::super::cartesian;

    fn assert_length<T>(vectors: &Vec<Vec<T>>) {
        let expected_len: usize = vectors.iter().map(Vec::len).product();
        let actual_len: usize = cartesian::product(vectors)
            .collect::<Vec<Vec<&T>>>()
            .len();
        assert_eq!(expected_len, actual_len);
    }

    #[test]
    fn test_len() {
        let vectors = vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2], vec![3, 3, 3, 3]];
        assert_length(&vectors);
    }

    #[test]
    fn test_unequal_length() {
        let vectors = vec![vec![1, 1], vec![2, 2, 2, 2], vec![3]];
        assert_length(&vectors);
    }

    #[test]
    fn test_empty() {
        let one_is_empty = [vec![0; 3], vec![0; 3], vec![0; 0]];
        let empty_product: Vec<_> = cartesian::product(&one_is_empty).collect();
        assert_eq!(empty_product.len(), 0);
    }

    #[test]
    fn test_i32() {
        let numbers: Vec<Vec<u32>> = vec![vec![0, 16, 32, 48], vec![0, 4, 8, 12], vec![0, 1, 2, 3]];
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
