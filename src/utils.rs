pub fn slices_are_equal<T: core::cmp::PartialEq>(a: &[T], b: &[T]) -> bool {
    a.len() == b.len() && a.starts_with(b)
}
