#![allow(dead_code)]

use std::hash::Hash;

use crate::{FastHashMap as HashMap, FastHashMapExt};
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// OrderedMap is an insertion ordered map.
#[derive(Clone, Debug)]
pub struct OrderedMap<K, V> {
    keys: Vec<K>,
    mp: HashMap<K, V>,
}

impl<K, V> PartialEq for OrderedMap<K, V>
where
    K: Eq + Hash + PartialEq,
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.keys == other.keys && self.mp == other.mp
    }
}

impl<K, V> Eq for OrderedMap<K, V>
where
    K: Eq + Hash,
    V: Eq,
{
}

// noCopy may be embedded into structs which must not be copied
// after the first use.
//
// See https://golang.org/issues/8005#issuecomment-190753527
// for details.
pub struct NoCopy;

// Lock is a no-op used by -copylocks checker from `go vet`.
impl NoCopy {
    pub fn lock(&self) {}
    pub fn unlock(&self) {}
}

impl<K, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        Self {
            keys: Vec::new(),
            mp: HashMap::new(),
        }
    }
}

// NewOrderedMapWithSizeHint creates a new OrderedMap with a hint for the number of elements it will contain.
pub fn new_ordered_map_with_size_hint<K, V>(hint: usize) -> OrderedMap<K, V>
where
    K: Eq + Hash,
{
    new_map_with_size_hint(hint)
}

fn new_map_with_size_hint<K, V>(hint: usize) -> OrderedMap<K, V>
where
    K: Eq + Hash,
{
    OrderedMap {
        keys: Vec::with_capacity(hint),
        mp: HashMap::with_capacity(hint),
    }
}

pub struct MapEntry<K, V> {
    pub key: K,
    pub value: V,
}

pub fn new_ordered_map_from_list<K, V>(items: Vec<MapEntry<K, V>>) -> OrderedMap<K, V>
where
    K: Eq + Hash + Clone,
{
    let mut mp = new_ordered_map_with_size_hint(items.len());
    for item in items {
        mp.set(item.key, item.value);
    }
    mp
}

impl<K, V> OrderedMap<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn with_size_hint(hint: usize) -> Self {
        new_ordered_map_with_size_hint(hint)
    }

    pub fn from_list(items: Vec<MapEntry<K, V>>) -> Self {
        new_ordered_map_from_list(items)
    }

    // Set sets a key-value pair in the map.
    pub fn set(&mut self, key: K, value: V) {
        if !self.mp.contains_key(&key) {
            self.keys.push(key.clone());
        }
        self.mp.insert(key, value);
    }

    // Get retrieves a value from the map.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.mp.get(key)
    }

    // GetOrZero retrieves a value from the map, or returns the zero value of the value type if the key is not present.
    pub fn get_or_zero(&self, key: &K) -> V
    where
        V: Default + Clone,
    {
        self.mp.get(key).cloned().unwrap_or_default()
    }

    // EntryAt retrieves the key-value pair at the specified index.
    pub fn entry_at(&self, index: isize) -> Option<(&K, &V)> {
        if index < 0 || index as usize >= self.keys.len() {
            return None;
        }

        let key = &self.keys[index as usize];
        let value = &self.mp[key];
        Some((key, value))
    }

    // Has returns true if the map contains the key.
    pub fn has(&self, key: &K) -> bool {
        self.mp.contains_key(key)
    }

    // Delete removes a key-value pair from the map.
    pub fn delete(&mut self, key: &K) -> Option<V> {
        let v = self.mp.remove(key)?;

        if let Some(i) = self.keys.iter().position(|existing| existing == key) {
            // If we're just removing the first or last element, avoid shifting everything around.
            if i == 0 {
                self.keys.remove(0);
            } else if i == self.keys.len() - 1 {
                self.keys.pop();
            } else {
                self.keys.remove(i);
            }
        }

        Some(v)
    }

    // Keys returns an iterator over the keys in the map.
    // A slice of the keys can be obtained by calling `slices.Collect`.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        // We use the backing key vector here to preserve insertion order.
        self.keys.iter()
    }

    // Values returns an iterator over the values in the map.
    // A slice of the values can be obtained by calling `slices.Collect`.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        // We use the backing key vector here to preserve insertion order.
        self.keys.iter().map(|key| &self.mp[key])
    }

    // Entries returns an iterator over the key-value pairs in the map.
    pub fn entries(&self) -> impl Iterator<Item = (&K, &V)> {
        // We use the backing key vector here to preserve insertion order.
        self.keys.iter().map(|key| (key, &self.mp[key]))
    }

    // Clear removes all key-value pairs from the map.
    // The space allocated for the map will be reused.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.mp.clear();
    }

    // Size returns the number of key-value pairs in the map.
    pub fn size(&self) -> usize {
        self.keys.len()
    }

    pub fn clone_map(&self) -> OrderedMap<K, V>
    where
        V: Clone,
    {
        self.clone_inner()
    }

    fn clone_inner(&self) -> OrderedMap<K, V>
    where
        V: Clone,
    {
        OrderedMap {
            keys: self.keys.clone(),
            mp: self.mp.clone(),
        }
    }
}

impl<K, V> Serialize for OrderedMap<K, V>
where
    K: Eq + Hash + Clone + OrderedMapJsonKey,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.keys.len()))?;
        for key in &self.keys {
            let key_string = resolve_key_name(key).map_err(serde::ser::Error::custom)?;
            map.serialize_entry(&key_string, &self.mp[key])?;
        }
        map.end()
    }
}

impl<'de, K, V> Deserialize<'de> for OrderedMap<K, V>
where
    K: Eq + Hash + Clone + std::str::FromStr,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct OrderedMapVisitor<K, V> {
            marker: std::marker::PhantomData<(K, V)>,
        }

        impl<'de, K, V> Visitor<'de> for OrderedMapVisitor<K, V>
        where
            K: Eq + Hash + Clone + std::str::FromStr,
            V: Deserialize<'de>,
        {
            type Value = OrderedMap<K, V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an object")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut result = access
                    .size_hint()
                    .map(new_ordered_map_with_size_hint)
                    .unwrap_or_default();
                while let Some((key, value)) = access.next_entry::<String, V>()? {
                    let key = key.parse::<K>().map_err(|_| {
                        serde::de::Error::custom("cannot unmarshal object key into Map")
                    })?;
                    result.set(key, value);
                }
                Ok(result)
            }
        }

        deserializer.deserialize_map(OrderedMapVisitor {
            marker: std::marker::PhantomData,
        })
    }
}

impl<K, V> OrderedMap<K, V>
where
    K: Eq + Hash + Clone + OrderedMapJsonKey + std::str::FromStr,
    V: serde::Serialize + serde::de::DeserializeOwned + Clone,
{
    pub fn marshal_json_to(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    pub fn unmarshal_json_from(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        if value.is_null() {
            // By convention, to approximate the behavior of Unmarshal itself,
            // Unmarshalers implement UnmarshalJSON([]byte("null")) as a no-op.
            // https://pkg.go.dev/encoding/json#Unmarshaler
            return Ok(());
        }
        if !value.is_object() {
            return Err("cannot unmarshal non-object JSON value into Map".into());
        }
        let incoming: OrderedMap<K, V> = serde_json::from_str(text)?;
        for (key, value) in incoming.entries() {
            self.set(key.clone(), value.clone());
        }
        Ok(())
    }
}

pub trait OrderedMapJsonKey {
    fn resolve_key_name(&self) -> Result<String, String>;
}

impl OrderedMapJsonKey for String {
    fn resolve_key_name(&self) -> Result<String, String> {
        Ok(self.clone())
    }
}

impl OrderedMapJsonKey for str {
    fn resolve_key_name(&self) -> Result<String, String> {
        Ok(self.to_owned())
    }
}

macro_rules! impl_ordered_map_json_key_for_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl OrderedMapJsonKey for $ty {
                fn resolve_key_name(&self) -> Result<String, String> {
                    Ok(self.to_string())
                }
            }
        )*
    };
}

impl_ordered_map_json_key_for_int!(
    isize, i8, i16, i32, i64, i128, usize, u8, u16, u32, u64, u128,
);

pub fn resolve_key_name<K: OrderedMapJsonKey + ?Sized>(k: &K) -> Result<String, String> {
    k.resolve_key_name()
}

pub fn diff_ordered_maps<K, V>(
    m1: &OrderedMap<K, V>,
    m2: &OrderedMap<K, V>,
    mut on_added: impl FnMut(&K, &V),
    mut on_removed: impl FnMut(&K, &V),
    mut on_modified: impl FnMut(&K, &V, &V),
) where
    K: Eq + Hash + Clone,
    V: PartialEq,
{
    diff_ordered_maps_func(
        m1,
        m2,
        |a, b| a == b,
        &mut on_added,
        &mut on_removed,
        &mut on_modified,
    );
}

pub fn diff_ordered_maps_func<K, V>(
    m1: &OrderedMap<K, V>,
    m2: &OrderedMap<K, V>,
    mut equal_values: impl FnMut(&V, &V) -> bool,
    mut on_added: impl FnMut(&K, &V),
    mut on_removed: impl FnMut(&K, &V),
    mut on_modified: impl FnMut(&K, &V, &V),
) where
    K: Eq + Hash + Clone,
{
    for (k, v2) in m2.entries() {
        if m1.get(k).is_none() {
            on_added(k, v2);
        }
    }
    for (k, v1) in m1.entries() {
        if let Some(v2) = m2.get(k) {
            if !equal_values(v1, v2) {
                on_modified(k, v1, v2);
            }
        } else {
            on_removed(k, v1);
        }
    }
}
