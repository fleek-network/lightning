/// The backing representation
#[derive(Clone, Copy)]
enum FlatHashSliceRepr<'s> {
    Slice(&'s [u8]),
}
