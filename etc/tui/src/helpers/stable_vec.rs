/// An instance of a vector that can only be safely added to becasue a component is depending on the order
/// Changing the order of this vector in anyway is unsafe if you dont account for the components that is depending on it
/// 
/// This is used in something like [crate::widgets::context_list::ContextList]
pub struct StableVec<T>(Vec<T>);

impl<T> StableVec<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn push(&mut self, item: T) {
        self.0.push(item);
    }

    pub fn extend(&mut self, items: impl IntoIterator<Item = T>) {
        self.0.extend(items);
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.0.get_mut(index)
    }

    pub unsafe fn as_vec(&mut self) -> &mut Vec<T> {
        &mut self.0
    }

    pub unsafe fn into_inner(self) -> Vec<T> {
        self.0
    }
}

impl<T> From <Vec<T>> for StableVec<T> {
    fn from(vec: Vec<T>) -> Self {
        Self(vec)
    }
}

impl<T> Default for StableVec<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> std::ops::Deref for StableVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}