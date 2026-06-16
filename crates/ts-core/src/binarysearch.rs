use std::cmp::Ordering;

// BinarySearchUniqueFunc works like [slices.BinarySearchFunc], but avoids extra
// invocations of the comparison function by assuming that only one element
// in the slice could match the target. Also, unlike [slices.BinarySearchFunc],
// the comparison function is passed the current index of the element being
// compared, instead of the target element.
pub fn binary_search_unique_func<E>(
    x: &[E],
    mut cmp: impl FnMut(usize, &E) -> Ordering,
) -> (usize, bool) {
    let n = x.len();
    if n == 0 {
        return (0, false);
    }
    let mut low = 0usize;
    let mut high = n - 1;
    while low <= high {
        let middle = low + ((high - low) >> 1);
        match cmp(middle, &x[middle]) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => {
                if middle == 0 {
                    return (0, false);
                }
                high = middle - 1;
            }
            Ordering::Equal => return (middle, true),
        }
    }
    (low, false)
}
