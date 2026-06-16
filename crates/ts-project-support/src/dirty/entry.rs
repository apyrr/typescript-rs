#[derive(Clone)]
pub struct MapEntry<K, V>
where
    K: Default,
    V: Default,
{
    pub(crate) key: K,
    pub(crate) original: V,
    pub(crate) value: V,
    pub(crate) dirty: bool,
    pub(crate) delete: bool,
}

impl<K, V> MapEntry<K, V>
where
    K: Clone + Default,
    V: Clone + Default,
{
    pub fn key(&self) -> K {
        self.key.clone()
    }

    pub fn original(&self) -> V {
        self.original.clone()
    }

    pub fn value(&self) -> V {
        if self.delete {
            return V::default();
        }
        self.value.clone()
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }
}
