#[derive(PartialEq, Debug)]
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
    if list_a.len() == 0 && list_b.len() == 0 {
        return ListCompareResult::Identical;
    }
    if list_a.len() == 0 {
        return ListCompareResult::Missing(&list_b[0]);
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
    if list_a.len() < list_b.len() {
        return ListCompareResult::Missing(&list_b[list_a.len()]);
    }
    ListCompareResult::Identical
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(&[], &[], ListCompareResult::Identical)]
    #[case(&[], &[1], ListCompareResult::Missing(&1))]
    #[case(&[2], &[], ListCompareResult::Unexpected(&2))]
    #[case(&[2, 6], &[2, 4, 6], ListCompareResult::Missing(&4))]
    #[case(&[2, 4, 6], &[2, 6], ListCompareResult::Unexpected(&4))]
    #[case(&[2, 4], &[2, 4, 6], ListCompareResult::Missing(&6))]
    #[case(&[2, 4, 6], &[2, 4], ListCompareResult::Unexpected(&6))]
    #[case(&[2, 4, 6], &[2, 4, 6], ListCompareResult::Identical)]
    #[tokio::test]
    async fn simple_lists(
        #[case] list_a: &[i32],
        #[case] list_b: &[i32],
        #[case] expected_result: ListCompareResult<&i32>,
    ) {
        // WHEN
        let result = compare_lists(&list_a, &list_b, |e| e, |_, _| true);

        // THEN
        assert_eq!(result, expected_result);
    }

    #[tokio::test]
    async fn custom_key() {
        // GIVEN
        let list_a = [(1, "1"), (2, "2"), (3, "3"), (4, "4")];
        let list_b = [(1, "1"), (2, "2"), (3, "3"), (4, "5")];

        // WHEN
        let result = compare_lists(&list_a, &list_b, |e| &e.0, |e1, e2| e1.1 == e2.1);

        // THEN
        assert_eq!(result, ListCompareResult::Unequal(&(4, "5")));
    }

    #[tokio::test]
    async fn unexpected() {
        // GIVEN
        let list_a = [1, 2, 3, 4, 5];
        let list_b = [1, 2, 4];

        // WHEN
        let result = compare_lists(&list_a, &list_b, |e| e, |_, _| true);

        // THEN
        assert_eq!(result, ListCompareResult::Unexpected(&3));
    }

    #[tokio::test]
    async fn debug() {
        // GIVEN
        let result = ListCompareResult::<i32>::Identical;

        // WHEN
        let debug = format!("{:?}", result);

        // THEN
        assert_eq!(debug, "Identical");
    }
}
