use std::any::Any;

pub trait EqAny: Any {
    fn eq_any(&self, other: &dyn Any) -> bool;
}

impl<T: Any + PartialEq> EqAny for T {
    fn eq_any(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<T>().is_some_and(|value| self == value)
    }
}
