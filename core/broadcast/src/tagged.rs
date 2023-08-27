use derive_more::{Deref, DerefMut};

/// A container for tagged objects.
#[derive(Default)]
pub struct Tagger(usize);

#[derive(Deref, DerefMut)]
pub struct Tagged<T> {
    tag: usize,
    #[deref]
    #[deref_mut]
    value: T,
}

impl Tagger {
    #[inline]
    fn get_next_tag(&mut self) -> usize {
        let tag = self.0;
        self.0 += 1;
        tag
    }

    #[inline]
    pub fn tag2<A, B>(&mut self, a: A, b: B) -> (Tagged<A>, Tagged<B>) {
        let tag = self.get_next_tag();
        (Tagged { tag, value: a }, Tagged { tag, value: b })
    }
}

impl<T> Tagged<T> {
    #[inline]
    pub fn has_same_tag_as<B>(&self, value: &Tagged<B>) -> bool {
        self.tag == value.tag
    }
}
