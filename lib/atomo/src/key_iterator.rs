use std::marker::PhantomData;

pub struct KeyIterator<K> {
    key: PhantomData<K>,
}
