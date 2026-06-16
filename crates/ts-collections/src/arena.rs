use std::{
    fmt,
    hash::{Hash, Hasher},
    iter::{Enumerate, FusedIterator},
    marker::PhantomData,
    ops::{Index, IndexMut, Range, RangeInclusive},
};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawIdx(u32);

impl RawIdx {
    pub const fn from_u32(value: u32) -> Self {
        Self(value)
    }

    pub const fn into_u32(self) -> u32 {
        self.0
    }

    pub const fn into_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for RawIdx {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<RawIdx> for u32 {
    fn from(value: RawIdx) -> Self {
        value.0
    }
}

impl fmt::Debug for RawIdx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for RawIdx {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub struct Idx<T> {
    raw: RawIdx,
    _ty: PhantomData<fn() -> T>,
}

impl<T> Idx<T> {
    pub const fn from_raw(raw: RawIdx) -> Self {
        Self {
            raw,
            _ty: PhantomData,
        }
    }

    pub const fn into_raw(self) -> RawIdx {
        self.raw
    }
}

impl<T> Clone for Idx<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Idx<T> {}

impl<T> PartialEq for Idx<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T> Eq for Idx<T> {}

impl<T> PartialOrd for Idx<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Idx<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}

impl<T> Hash for Idx<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T> fmt::Debug for Idx<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let type_name = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or_default();
        write!(f, "Idx::<{}>({})", type_name, self.raw)
    }
}

pub struct IdxRange<T> {
    range: Range<u32>,
    _ty: PhantomData<fn() -> T>,
}

impl<T> IdxRange<T> {
    pub fn new(range: Range<Idx<T>>) -> Self {
        Self {
            range: range.start.into_raw().into_u32()..range.end.into_raw().into_u32(),
            _ty: PhantomData,
        }
    }

    pub fn new_inclusive(range: RangeInclusive<Idx<T>>) -> Self {
        Self {
            range: range.start().into_raw().into_u32()..range.end().into_raw().into_u32() + 1,
            _ty: PhantomData,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    pub fn start(&self) -> Idx<T> {
        Idx::from_raw(self.range.start.into())
    }

    pub fn end(&self) -> Idx<T> {
        Idx::from_raw(self.range.end.into())
    }
}

impl<T> Iterator for IdxRange<T> {
    type Item = Idx<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.range.next().map(|idx| Idx::from_raw(idx.into()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl<T> DoubleEndedIterator for IdxRange<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.range.next_back().map(|idx| Idx::from_raw(idx.into()))
    }
}

impl<T> ExactSizeIterator for IdxRange<T> {}

impl<T> FusedIterator for IdxRange<T> {}

impl<T> Clone for IdxRange<T> {
    fn clone(&self) -> Self {
        Self {
            range: self.range.clone(),
            _ty: PhantomData,
        }
    }
}

impl<T> PartialEq for IdxRange<T> {
    fn eq(&self, other: &Self) -> bool {
        self.range == other.range
    }
}

impl<T> Eq for IdxRange<T> {}

impl<T> fmt::Debug for IdxRange<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IdxRange").field(&self.range).finish()
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Arena<T> {
    data: Vec<T>,
}

impl<T> Arena<T> {
    pub const fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    pub fn alloc(&mut self, value: T) -> Idx<T> {
        let idx = self.next_idx();
        self.data.push(value);
        idx
    }

    pub fn alloc_many<I: IntoIterator<Item = T>>(&mut self, iter: I) -> IdxRange<T> {
        let start = self.next_idx();
        let iter = iter.into_iter();
        let (lower, upper) = iter.size_hint();
        let max_additional = upper.unwrap_or(lower);
        Self::assert_idx_fits_u32(
            self.data
                .len()
                .checked_add(max_additional)
                .expect("arena length exceeds usize index space"),
        );
        self.data.reserve(lower);
        for value in iter {
            self.alloc(value);
        }
        let end = self.next_idx();
        IdxRange::new(start..end)
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn get(&self, idx: Idx<T>) -> Option<&T> {
        self.data.get(idx.into_raw().into_usize())
    }

    pub fn get_mut(&mut self, idx: Idx<T>) -> Option<&mut T> {
        self.data.get_mut(idx.into_raw().into_usize())
    }

    pub fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (Idx<T>, &T)> + DoubleEndedIterator + Clone {
        self.data
            .iter()
            .enumerate()
            .map(|(idx, value)| (Self::idx_from_usize(idx), value))
    }

    pub fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = (Idx<T>, &mut T)> + DoubleEndedIterator {
        self.data
            .iter_mut()
            .enumerate()
            .map(|(idx, value)| (Self::idx_from_usize(idx), value))
    }

    pub fn values(&self) -> impl ExactSizeIterator<Item = &T> + DoubleEndedIterator {
        self.data.iter()
    }

    pub fn values_mut(&mut self) -> impl ExactSizeIterator<Item = &mut T> + DoubleEndedIterator {
        self.data.iter_mut()
    }

    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }

    fn assert_idx_fits_u32(idx: usize) {
        assert!(
            u32::try_from(idx).is_ok(),
            "arena length exceeds u32 index space"
        );
    }

    fn idx_from_usize(idx: usize) -> Idx<T> {
        Self::assert_idx_fits_u32(idx);
        Idx::from_raw((idx as u32).into())
    }

    fn next_idx(&self) -> Idx<T> {
        Self::idx_from_usize(self.data.len())
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> fmt::Debug for Arena<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Arena")
            .field("len", &self.len())
            .field("data", &self.data)
            .finish()
    }
}

impl<T> Index<Idx<T>> for Arena<T> {
    type Output = T;

    fn index(&self, index: Idx<T>) -> &Self::Output {
        &self.data[index.into_raw().into_usize()]
    }
}

impl<T> IndexMut<Idx<T>> for Arena<T> {
    fn index_mut(&mut self, index: Idx<T>) -> &mut Self::Output {
        &mut self.data[index.into_raw().into_usize()]
    }
}

impl<T> Index<IdxRange<T>> for Arena<T> {
    type Output = [T];

    fn index(&self, range: IdxRange<T>) -> &Self::Output {
        &self.data[range.range.start as usize..range.range.end as usize]
    }
}

impl<T> Extend<T> for Arena<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.alloc(value);
        }
    }
}

impl<T> FromIterator<T> for Arena<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let data = Vec::from_iter(iter);
        Self::assert_idx_fits_u32(data.len());
        Self { data }
    }
}

pub struct ArenaIntoIter<T>(Enumerate<std::vec::IntoIter<T>>);

impl<T> Iterator for ArenaIntoIter<T> {
    type Item = (Idx<T>, T);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|(idx, value)| (Arena::<T>::idx_from_usize(idx), value))
    }
}

impl<T> DoubleEndedIterator for ArenaIntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0
            .next_back()
            .map(|(idx, value)| (Arena::<T>::idx_from_usize(idx), value))
    }
}

impl<T> IntoIterator for Arena<T> {
    type Item = (Idx<T>, T);
    type IntoIter = ArenaIntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        ArenaIntoIter(self.data.into_iter().enumerate())
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ArenaMap<IDX, V> {
    values: Vec<Option<V>>,
    _idx: PhantomData<fn() -> IDX>,
}

impl<T, V> ArenaMap<Idx<T>, V> {
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            _idx: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            values: Vec::with_capacity(capacity),
            _idx: PhantomData,
        }
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn contains_idx(&self, idx: Idx<T>) -> bool {
        self.get(idx).is_some()
    }

    pub fn insert(&mut self, idx: Idx<T>, value: V) -> Option<V> {
        let idx = idx.into_raw().into_usize();
        if self.values.len() <= idx {
            self.values.resize_with(idx + 1, || None);
        }
        self.values[idx].replace(value)
    }

    pub fn get(&self, idx: Idx<T>) -> Option<&V> {
        self.values
            .get(idx.into_raw().into_usize())
            .and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, idx: Idx<T>) -> Option<&mut V> {
        self.values
            .get_mut(idx.into_raw().into_usize())
            .and_then(Option::as_mut)
    }

    pub fn remove(&mut self, idx: Idx<T>) -> Option<V> {
        self.values
            .get_mut(idx.into_raw().into_usize())
            .and_then(Option::take)
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> {
        self.values.iter().filter_map(Option::as_ref)
    }

    pub fn values_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut V> {
        self.values.iter_mut().filter_map(Option::as_mut)
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (Idx<T>, &V)> {
        self.values
            .iter()
            .enumerate()
            .filter_map(|(idx, value)| Some((Idx::from_raw((idx as u32).into()), value.as_ref()?)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Idx<T>, &mut V)> {
        self.values
            .iter_mut()
            .enumerate()
            .filter_map(|(idx, value)| Some((Idx::from_raw((idx as u32).into()), value.as_mut()?)))
    }

    pub fn shrink_to_fit(&mut self) {
        let len = self
            .values
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |idx| idx + 1);
        self.values.truncate(len);
        self.values.shrink_to_fit();
    }
}

impl<T, V> Default for ArenaMap<Idx<T>, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, V> fmt::Debug for ArenaMap<Idx<T>, V>
where
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ArenaMap")
            .field("values", &self.values)
            .finish()
    }
}

impl<T, V> Index<Idx<T>> for ArenaMap<Idx<T>, V> {
    type Output = V;

    fn index(&self, index: Idx<T>) -> &Self::Output {
        self.get(index).expect("missing arena map value")
    }
}

impl<T, V> IndexMut<Idx<T>> for ArenaMap<Idx<T>, V> {
    fn index_mut(&mut self, index: Idx<T>) -> &mut Self::Output {
        self.get_mut(index).expect("missing arena map value")
    }
}

impl<T, V> Extend<(Idx<T>, V)> for ArenaMap<Idx<T>, V> {
    fn extend<I: IntoIterator<Item = (Idx<T>, V)>>(&mut self, iter: I) {
        for (idx, value) in iter {
            self.insert(idx, value);
        }
    }
}

impl<T, V> FromIterator<(Idx<T>, V)> for ArenaMap<Idx<T>, V> {
    fn from_iter<I: IntoIterator<Item = (Idx<T>, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

pub struct ArenaMapIntoIter<T, V> {
    iter: Enumerate<std::vec::IntoIter<Option<V>>>,
    _ty: PhantomData<fn() -> T>,
}

impl<T, V> Iterator for ArenaMapIntoIter<T, V> {
    type Item = (Idx<T>, V);

    fn next(&mut self) -> Option<Self::Item> {
        for (idx, value) in self.iter.by_ref() {
            if let Some(value) = value {
                return Some((Idx::from_raw((idx as u32).into()), value));
            }
        }
        None
    }
}

impl<T, V> DoubleEndedIterator for ArenaMapIntoIter<T, V> {
    fn next_back(&mut self) -> Option<Self::Item> {
        for (idx, value) in self.iter.by_ref().rev() {
            if let Some(value) = value {
                return Some((Idx::from_raw((idx as u32).into()), value));
            }
        }
        None
    }
}

impl<T, V> IntoIterator for ArenaMap<Idx<T>, V> {
    type Item = (Idx<T>, V);
    type IntoIter = ArenaMapIntoIter<T, V>;

    fn into_iter(self) -> Self::IntoIter {
        ArenaMapIntoIter {
            iter: self.values.into_iter().enumerate(),
            _ty: PhantomData,
        }
    }
}
