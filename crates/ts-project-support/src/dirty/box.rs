use super::{Cloneable, Value};

pub struct Box<T>
where
    T: Clone + Cloneable<T> + Default,
{
    original: T,
    value: T,
    dirty: bool,
    delete: bool,
}

impl<T> Clone for Box<T>
where
    T: Clone + Cloneable<T> + Default,
{
    fn clone(&self) -> Self {
        Self {
            original: self.original.clone(),
            value: self.value.clone(),
            dirty: self.dirty,
            delete: self.delete,
        }
    }
}

pub fn new_box<T>(original: T) -> Box<T>
where
    T: Clone + Cloneable<T> + Default,
{
    Box {
        value: original.clone(),
        original,
        dirty: false,
        delete: false,
    }
}

impl<T> Value<T> for Box<T>
where
    T: Clone + Cloneable<T> + Default,
{
    fn value(&self) -> T {
        if self.delete {
            return T::default();
        }
        self.value.clone()
    }

    fn original(&self) -> T {
        self.original.clone()
    }

    fn dirty(&self) -> bool {
        self.dirty
    }

    fn change(&mut self, apply: impl FnOnce(T)) {
        if !self.dirty {
            self.value = self.value.clone_value();
            self.dirty = true;
        }
        apply(self.value.clone());
    }

    fn change_if(&mut self, cond: impl FnOnce(T) -> bool, apply: impl FnOnce(T)) -> bool {
        if cond(self.value.clone()) {
            self.change(apply);
            return true;
        }
        false
    }

    fn delete(&mut self) {
        self.delete = true;
    }

    fn locked(&self, f: impl FnOnce(&Self)) {
        f(self);
    }
}

impl<T> Box<T>
where
    T: Clone + Cloneable<T> + Default,
{
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.delete = false;
        self.dirty = true;
    }

    pub fn finalize(&self) -> (T, bool) {
        (self.value(), self.dirty || self.delete)
    }
}
