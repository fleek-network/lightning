use atomo::mt::{MtAtomo, MtAtomoBuilder, QueryPerm, UpdatePerm};
use atomo::DefaultSerdeBackend;

pub struct Env<P> {
    inner: MtAtomo<P>,
}
