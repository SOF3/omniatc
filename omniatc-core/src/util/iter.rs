/// Collects from an iterator, only storing the last yielded item.
pub struct TakeLast<T>(pub Option<T>);

impl<T> Default for TakeLast<T> {
    fn default() -> Self { TakeLast(None) }
}

impl<T> FromIterator<T> for TakeLast<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut last = None;
        for item in iter {
            last = Some(item);
        }
        TakeLast(last)
    }
}

impl<T> Extend<T> for TakeLast<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) { self.0 = iter.into_iter().last(); }
}
