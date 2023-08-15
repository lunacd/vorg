pub enum ListCompareResult<T> {
    Missing(T),
    Unexpected(T),
    Unequal(T),
    Identical,
}

/// Compare `list_a` against `list_b`, both sorted by a key given by `compare_key` in ascending
/// order.
///
/// If an item exists in `list_b` but not in `list_a`, returns `ListCompareResult::Missing` with
/// the item missing.
/// If an item does not exist in `list_b` but exists in `list_a`, returns
/// `ListCompareResult::Unexpected`.
/// If an item exists in both, the item is checked with `equality_check`. If those two items does
/// not pass, returns `ListCompareResult::Unequal`, containing the item in `list_b`.
///
/// Only the first detected problem is returned.
pub fn compare_lists<'a, T, C>(
    list_a: &'a [T],
    list_b: &'a [T],
    compare_key: fn(&T) -> &C,
    equality_check: fn(&T, &T) -> bool,
) -> ListCompareResult<&'a T>
where
    T: Clone,
    C: PartialOrd + Clone,
{
    if list_a.len() < list_b.len() {
        return ListCompareResult::Missing(&list_b[list_a.len()]);
    }
    for (index, item_a) in list_a.iter().enumerate() {
        if index >= list_b.len() {
            return ListCompareResult::Unexpected(item_a);
        }
        let item_b = &list_b[index];
        let key_a = compare_key(item_a);
        let key_b = compare_key(item_b);
        if key_a > key_b {
            return ListCompareResult::Missing(item_b);
        }
        if key_a < key_b {
            return ListCompareResult::Unexpected(item_a);
        }
        if !equality_check(item_a, item_b) {
            return ListCompareResult::Unequal(item_b);
        }
    }
    ListCompareResult::Identical
}
