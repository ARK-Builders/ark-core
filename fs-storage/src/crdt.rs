// Currently, we have three structures: Tags (HashSet), Properties (HashSet), Score (int).
// In fact, HashSet already implements a union function,
// so only a special function for integers is needed.
// CRDTs can be considered later when we need to add structures that require
// more powerful combine semantics.

pub trait CRDT<V> {
    fn neutral() -> V;

    fn combine(a: &V, b: &V) -> V;

    fn combine_all<I: IntoIterator<Item = V>>(values: I) -> V
    where
        V: Clone,
    {
        values
            .into_iter()
            .fold(Self::neutral(), |acc, val| Self::combine(&acc, &val))
    }
}
