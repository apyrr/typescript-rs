use std::hash::Hash;

use crate::{FastHashSet as HashSet, FastHashSetExt};

// OrderedSet an insertion ordered set.
#[derive(Clone)]
pub struct OrderedSet<T> {
    values: Vec<T>,
    set: HashSet<T>,
}

impl<T> Default for OrderedSet<T> {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            set: HashSet::new(),
        }
    }
}

// NewOrderedSetWithSizeHint creates a new OrderedSet with a hint for the number of elements it will contain.
pub fn new_ordered_set_with_size_hint<T>(hint: usize) -> OrderedSet<T>
where
    T: Eq + Hash,
{
    OrderedSet {
        values: Vec::with_capacity(hint),
        set: HashSet::with_capacity(hint),
    }
}

impl<T> OrderedSet<T>
where
    T: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    // Add adds a value to the set.
    pub fn add(&mut self, value: T) {
        if self.set.insert(value.clone()) {
            self.values.push(value);
        }
    }

    // Has returns true if the set contains the value.
    pub fn has(&self, value: &T) -> bool {
        self.set.contains(value)
    }

    // Delete removes a value from the set.
    pub fn delete(&mut self, value: &T) -> bool {
        if !self.set.remove(value) {
            return false;
        }
        self.values.retain(|existing| existing != value);
        true
    }

    // Values returns an iterator over the values in the set.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.values.iter()
    }

    // Clear removes all elements from the set.
    // The space allocated for the set will be reused.
    pub fn clear(&mut self) {
        self.values.clear();
        self.set.clear();
    }

    // Size returns the number of elements in the set.
    pub fn size(&self) -> usize {
        self.values.len()
    }

    // Clone returns a shallow copy of the set.
    pub fn clone_set(&self) -> OrderedSet<T> {
        self.clone()
    }

    #[cfg(test)]
    pub(crate) fn capacities(&self) -> (usize, usize) {
        (self.values.capacity(), self.set.capacity())
    }
}
