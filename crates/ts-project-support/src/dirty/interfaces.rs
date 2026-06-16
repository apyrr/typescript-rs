pub trait Cloneable<T> {
    fn clone_value(&self) -> T;
}

pub trait Value<T> {
    fn value(&self) -> T;
    fn original(&self) -> T;
    fn dirty(&self) -> bool;
    fn change(&mut self, apply: impl FnOnce(T));
    fn change_if(&mut self, cond: impl FnOnce(T) -> bool, apply: impl FnOnce(T)) -> bool;
    fn delete(&mut self);
    fn locked(&self, f: impl FnOnce(&Self));
}
