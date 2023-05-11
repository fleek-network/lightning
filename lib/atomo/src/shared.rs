use std::borrow::Borrow;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Reference to a value of type `T` that can be owned as well, in a way this type
/// is similar to [`Cow`] except we don't care about *clone-on-write*.
///
/// This type tries to not get in the way as much as possible by shadow implementing
/// most of the things `T` already implements, but you can always use `(&*shared)`
/// syntax to get to `&T` as fast as you can.
#[derive(Copy)]
pub struct Shared<'a, T>(SharedInner<'a, T>);

impl<'a, T> Shared<'a, T> {
    /// Create a new shared value from the given reference.
    #[inline]
    pub fn new(value: &'a T) -> Self {
        Self(SharedInner::Reference(value))
    }

    /// Create a new owned value.
    pub fn owned(value: T) -> Self {
        Self(SharedInner::Owned(value))
    }

    /// Converts the shared reference to the inner type `T`. This is done by simply returning
    /// the data if this shared reference is owned, otherwise a clone is performed.
    #[inline]
    pub fn into_inner(self) -> T
    where
        T: Clone,
    {
        match self.0 {
            SharedInner::Reference(value) => value.clone(),
            SharedInner::Owned(value) => value,
        }
    }
}

#[derive(Copy)]
enum SharedInner<'a, T> {
    /// The value is a reference to an existing value.
    Reference(&'a T),
    /// The value is owned.
    Owned(T),
}

impl<'a, T> Deref for Shared<'a, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        match &self.0 {
            SharedInner::Reference(t) => *t,
            SharedInner::Owned(t) => t,
        }
    }
}

impl<'a, T> AsRef<T> for Shared<'a, T> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        match &self.0 {
            SharedInner::Reference(t) => *t,
            SharedInner::Owned(t) => t,
        }
    }
}

impl<'a, T> Borrow<T> for Shared<'a, T> {
    #[inline]
    fn borrow(&self) -> &T {
        match &self.0 {
            SharedInner::Reference(t) => *t,
            SharedInner::Owned(t) => t,
        }
    }
}

impl<'a, T: Clone> Clone for Shared<'a, T> {
    fn clone(&self) -> Self {
        match &self.0 {
            SharedInner::Reference(t) => Self(SharedInner::Reference(*t)),
            SharedInner::Owned(t) => Self(SharedInner::Owned(t.clone())),
        }
    }
}

impl<'a, T: Clone> Clone for SharedInner<'a, T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        match self {
            SharedInner::Reference(t) => SharedInner::Reference(*t),
            SharedInner::Owned(t) => SharedInner::Owned(t.clone()),
        }
    }
}

impl<'a, T: Debug> Debug for Shared<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            SharedInner::Reference(t) => (*t).fmt(f),
            SharedInner::Owned(t) => t.fmt(f),
        }
    }
}

impl<'a, T: Display> Display for Shared<'a, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            SharedInner::Reference(t) => (*t).fmt(f),
            SharedInner::Owned(t) => t.fmt(f),
        }
    }
}

impl<'a, T: Hash> Hash for Shared<'a, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_ref().hash(state)
    }
}

impl<'a, T: PartialEq> PartialEq<Shared<'a, T>> for Shared<'a, T> {
    #[inline]
    fn eq(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().eq(other.as_ref())
    }

    #[inline]
    fn ne(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().ne(other.as_ref())
    }
}

impl<'a, T: PartialEq> PartialEq<T> for Shared<'a, T> {
    #[inline]
    fn eq(&self, other: &T) -> bool {
        self.as_ref().eq(other)
    }

    #[inline]
    fn ne(&self, other: &T) -> bool {
        self.as_ref().ne(other)
    }
}

impl<'a, T: PartialOrd> PartialOrd<Shared<'a, T>> for Shared<'a, T> {
    #[inline]
    fn partial_cmp(&self, other: &Shared<'a, T>) -> Option<std::cmp::Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }

    #[inline]
    fn lt(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().lt(other.as_ref())
    }

    #[inline]
    fn le(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().le(other.as_ref())
    }

    #[inline]
    fn gt(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().gt(other.as_ref())
    }

    #[inline]
    fn ge(&self, other: &Shared<'a, T>) -> bool {
        self.as_ref().ge(other.as_ref())
    }
}

impl<'a, T: PartialOrd> PartialOrd<T> for Shared<'a, T> {
    #[inline]
    fn partial_cmp(&self, other: &T) -> Option<std::cmp::Ordering> {
        self.as_ref().partial_cmp(other)
    }

    #[inline]
    fn lt(&self, other: &T) -> bool {
        self.as_ref().lt(other)
    }

    #[inline]
    fn le(&self, other: &T) -> bool {
        self.as_ref().le(other)
    }

    #[inline]
    fn gt(&self, other: &T) -> bool {
        self.as_ref().gt(other)
    }

    #[inline]
    fn ge(&self, other: &T) -> bool {
        self.as_ref().ge(other)
    }
}

impl<'a, T: Eq> Eq for Shared<'a, T> {}

impl<'a, T: Ord> Ord for Shared<'a, T> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl<'a, T: std::ops::Add<T, Output = T> + Copy> std::ops::Add<T> for Shared<'a, T> {
    type Output = T;

    #[inline]
    fn add(self, rhs: T) -> Self::Output {
        let lhs = *self.as_ref();
        lhs.add(rhs)
    }
}

impl<'a, T: std::ops::Add<T, Output = T> + Copy> std::ops::Add<Shared<'a, T>> for Shared<'a, T> {
    type Output = T;

    #[inline]
    fn add(self, rhs: Shared<'a, T>) -> Self::Output {
        let lhs = *self.as_ref();
        let rhs = *rhs.as_ref();
        lhs.add(rhs)
    }
}

impl<'a, T: std::ops::Sub<T, Output = T> + Copy> std::ops::Sub<T> for Shared<'a, T> {
    type Output = T;

    #[inline]
    fn sub(self, rhs: T) -> Self::Output {
        let lhs = *self.as_ref();
        lhs.sub(rhs)
    }
}

impl<'a, T: std::ops::Sub<T, Output = T> + Copy> std::ops::Sub<Shared<'a, T>> for Shared<'a, T> {
    type Output = T;

    #[inline]
    fn sub(self, rhs: Shared<'a, T>) -> Self::Output {
        let lhs = *self.as_ref();
        let rhs = *rhs.as_ref();
        lhs.sub(rhs)
    }
}

impl<'a, T: std::ops::Mul<T, Output = T> + Copy> std::ops::Mul<T> for Shared<'a, T> {
    type Output = T;

    #[inline]
    fn mul(self, rhs: T) -> Self::Output {
        let lhs = *self.as_ref();
        lhs.mul(rhs)
    }
}

impl<'a, T: std::ops::Mul<T, Output = T> + Copy> std::ops::Mul<Shared<'a, T>> for Shared<'a, T> {
    type Output = T;

    #[inline]
    fn mul(self, rhs: Shared<'a, T>) -> Self::Output {
        let lhs = *self.as_ref();
        let rhs = *rhs.as_ref();
        lhs.mul(rhs)
    }
}

impl<'a, T: std::ops::Div<T, Output = T> + Copy> std::ops::Div<T> for Shared<'a, T> {
    type Output = T;

    #[inline]
    fn div(self, rhs: T) -> Self::Output {
        let lhs = *self.as_ref();
        lhs.div(rhs)
    }
}

impl<'a, T: std::ops::Div<T, Output = T> + Copy> std::ops::Div<Shared<'a, T>> for Shared<'a, T> {
    type Output = T;

    #[inline]
    fn div(self, rhs: Shared<'a, T>) -> Self::Output {
        let lhs = *self.as_ref();
        let rhs = *rhs.as_ref();
        lhs.div(rhs)
    }
}

impl<'a, T: Serialize> Serialize for Shared<'a, T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        T::serialize(self.as_ref(), serializer)
    }
}

impl<'a, 'de, T: Deserialize<'de>> Deserialize<'de> for Shared<'a, T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Shared::owned(T::deserialize(deserializer)?))
    }
}
