
use std::iter;

pub struct Product<'a, T: 'a> {
    collections: &'a [Vec<T>],
    iterators: Vec<::std::slice::Iter<'a, T>>,
    next_item: Vec<Option<&'a T>>,
}

impl<'a, T> Product<'a, T> {
    pub fn new(collections: &'a [Vec<T>]) -> Self {
        // Create an unitialized object.
        // we have to fill `iterators` and `next_item`.
        let len = collections.len();
        let mut product = Self {
            collections: collections,
            iterators: Vec::with_capacity(len),
            next_item: iter::repeat(None).take(len).collect(),
        };

        // Create one brand-new iterator per collection.
        for collection in product.collections {
            product.iterators.push(collection.into_iter());
        }

        // Fill `next_item`, which is full of Nones until now.
        product.fill_up_next_item();

        product
    }

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
                Some(_) => {
                    break;
                }
                None => {
                    *iterator = collection.iter();
                }
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

impl<'a, T> Iterator for Product<'a, T> {
    type Item = Vec<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next_item.iter().any(Option::is_none) {
            // If any element is None, it means at least one of the
            // sub-iterators is exhausted and this iterator is
            // exhausted as a whole. We are done then.
            return None;
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_len() {
        let all_collections = vec![vec![1, 1, 1, 1], vec![2, 2, 2, 2], vec![3, 3, 3, 3]];
        let expected_len = all_collections.iter().map(|v| v.len()).product();
        let combinations: Vec<_> = Product::new(&all_collections).collect();
        assert_eq!(combinations.len(), expected_len);
    }

    #[test]
    fn test_i32() {
        let all_collections: Vec<Vec<u32>> =
            vec![vec![0, 16, 32, 48], vec![0, 4, 8, 12], vec![0, 1, 2, 3]];
        let expected: Vec<u32> = (0..64u32).collect();
        let actual: Vec<u32> = Product::new(&all_collections)
            .map(|combos| combos.into_iter().sum())
            .collect();
        assert_eq!(expected, actual);
    }
    #[test]
    fn test_slices() {
        let bits: [[u8; 2]; 4] = [[0, 8], [0, 4], [0, 2], [0, 1]];
        //~ let expected: Vec<u32> = (0..64u32).collect();
        //~ let actual: Vec<u32> = Product::new(&all_collections)
        //~ .map(|combos| combos.into_iter().sum())
        //~ .collect();
        //~ assert_eq!(expected, actual);
    }
}
