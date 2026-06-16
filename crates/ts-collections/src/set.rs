#![allow(dead_code)]

use std::hash::Hash;

use crate::{FastHashSet as HashSet, FastHashSetExt};

#[derive(Clone, Debug)]
pub struct Set<T> {
    pub m: Option<HashSet<T>>,
}

impl<T> PartialEq for Set<T>
where
    T: Eq + Hash,
{
    fn eq(&self, other: &Self) -> bool {
        self.m == other.m
    }
}

impl<T> Eq for Set<T> where T: Eq + Hash {}

impl<T> Default for Set<T> {
    fn default() -> Self {
        Self { m: None }
    }
}

// NewSetWithSizeHint creates a new Set with a hint for the number of elements it will contain.
pub fn new_set_with_size_hint<T>(hint: usize) -> Set<T>
where
    T: Eq + Hash,
{
    Set {
        m: Some(HashSet::with_capacity(hint)),
    }
}

impl<T> Set<T>
where
    T: Eq + Hash,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_from_items(items: impl IntoIterator<Item = T>) -> Self {
        let mut set = Self::default();
        for item in items {
            set.add(item);
        }
        set
    }

    pub fn has(&self, key: &T) -> bool {
        self.m.as_ref().is_some_and(|m| m.contains(key))
    }

    pub fn add(&mut self, key: T) {
        self.m.get_or_insert_with(HashSet::new).insert(key);
    }

    pub fn delete(&mut self, key: &T) {
        if let Some(m) = &mut self.m {
            m.remove(key);
        }
    }

    pub fn len(&self) -> usize {
        self.m.as_ref().map_or(0, HashSet::len)
    }

    pub fn is_empty(&self) -> bool {
        self.m.as_ref().is_none_or(HashSet::is_empty)
    }

    pub fn keys(&self) -> Option<&HashSet<T>> {
        self.m.as_ref()
    }

    pub fn clear(&mut self) {
        if let Some(m) = &mut self.m {
            m.clear();
        }
    }

    // Returns true if the key was not already present in the set.
    pub fn add_if_absent(&mut self, key: T) -> bool {
        if self.has(&key) {
            return false;
        }
        self.add(key);
        true
    }
}

impl<T> Set<T>
where
    T: Eq + Hash + Clone,
{
    pub fn clone_set(&self) -> Set<T> {
        self.clone()
    }

    pub fn union(&mut self, other: &Set<T>) {
        if self.is_empty() && other.is_empty() {
            return;
        }
        if self.m.is_none() {
            self.m = other.m.clone();
            return;
        }
        if let (Some(m), Some(other_m)) = (&mut self.m, &other.m) {
            m.extend(other_m.iter().cloned());
        }
    }

    pub fn unioned_with(&self, other: Option<&Set<T>>) -> Set<T> {
        let mut result = self.clone();
        if let Some(other) = other {
            if result.m.is_none() {
                result.m = Some(HashSet::with_capacity(other.len()));
            }
            if let (Some(m), Some(other_m)) = (&mut result.m, &other.m) {
                m.extend(other_m.iter().cloned());
            }
        }
        result
    }

    pub fn equals(&self, other: &Set<T>) -> bool {
        self.len() == other.len() && self.is_subset_of(other)
    }

    pub fn is_subset_of(&self, other: &Set<T>) -> bool {
        self.m
            .as_ref()
            .is_none_or(|m| m.iter().all(|key| other.has(key)))
    }

    pub fn intersects(&self, other: &Set<T>) -> bool {
        match (&self.m, &other.m) {
            (Some(m), Some(_)) => m.iter().any(|key| other.has(key)),
            _ => false,
        }
    }
}

pub trait SetOptionExt<'a, T> {
    fn has(self, key: &T) -> bool;
    fn len(self) -> usize;
    fn keys(self) -> Option<&'a HashSet<T>>;
    fn clone_set(self) -> Option<Set<T>>;
    fn unioned_with(self, other: Option<&Set<T>>) -> Option<Set<T>>;
    fn equals(self, other: Option<&Set<T>>) -> bool;
    fn subset_of(self, other: Option<&Set<T>>) -> bool;
    fn intersects(self, other: Option<&Set<T>>) -> bool;
}

impl<'a, T> SetOptionExt<'a, T> for Option<&'a Set<T>>
where
    T: Eq + Hash + Clone,
{
    fn has(self, key: &T) -> bool {
        self.is_some_and(|s| s.has(key))
    }

    fn len(self) -> usize {
        self.map_or(0, Set::len)
    }

    fn keys(self) -> Option<&'a HashSet<T>> {
        self.and_then(Set::keys)
    }

    fn clone_set(self) -> Option<Set<T>> {
        self.map(Set::clone_set)
    }

    fn unioned_with(self, other: Option<&Set<T>>) -> Option<Set<T>> {
        if self.is_none() && other.is_none() {
            return None;
        }
        let mut result = self.map_or_else(Set::default, Set::clone_set);
        if let Some(other) = other {
            if result.m.is_none() {
                result.m = Some(HashSet::with_capacity(other.len()));
            }
            if let (Some(m), Some(other_m)) = (&mut result.m, &other.m) {
                m.extend(other_m.iter().cloned());
            }
        }
        Some(result)
    }

    fn equals(self, other: Option<&Set<T>>) -> bool {
        match (self, other) {
            (None, None) => true,
            (Some(s), Some(other)) => s.equals(other),
            _ => false,
        }
    }

    fn subset_of(self, other: Option<&Set<T>>) -> bool {
        match self {
            None => true,
            Some(s) => {
                s.m.as_ref()
                    .is_none_or(|m| m.iter().all(|key| other.has(key)))
            }
        }
    }

    fn intersects(self, other: Option<&Set<T>>) -> bool {
        match (self, other) {
            (Some(s), Some(other)) => s.intersects(other),
            _ => false,
        }
    }
}

pub trait SetOptionMutExt<T> {
    fn union(self, other: Option<&Set<T>>);
    fn clear(self);
}

impl<T> SetOptionMutExt<T> for Option<&mut Set<T>>
where
    T: Eq + Hash + Clone,
{
    fn union(self, other: Option<&Set<T>>) {
        if self.as_ref().map_or(0, |s| s.len()) == 0 && other.len() == 0 {
            return;
        }
        let Some(s) = self else {
            panic!("cannot modify nil Set");
        };
        let Some(other) = other else {
            panic!("nil Set");
        };
        s.union(other);
    }

    fn clear(self) {
        if let Some(s) = self {
            s.clear();
        }
    }
}

pub fn new_set_from_items<T>(items: impl IntoIterator<Item = T>) -> Set<T>
where
    T: Eq + Hash + Clone,
{
    let mut s = Set::default();
    for item in items {
        s.add(item);
    }
    s
}
