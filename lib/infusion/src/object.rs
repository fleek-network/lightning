//! An implementation of a thin dynamically typed object which can be downcasted to the
//! concrete type manually. This can serve as a replacement of `Box<dyn Any>`.

use std::any::{type_name, TypeId};
use std::fmt::Debug;
use std::hash::Hash;
use std::mem::transmute;

/// A dynamically allocated object.
pub struct Object {
    /// The type tag for this object.
    tag: Tag,
    /// The raw pointer to the data.
    ptr: *mut (),
    /// The drop function that can drop this value.
    drop: fn(*mut ()),
}

/// A named [`TypeId`] that i
#[derive(Copy, Clone)]
pub struct Tag {
    /// The rust type id.
    type_id: TypeId,
    /// Human readable name of the type.
    type_name: &'static str,
}

impl Tag {
    /// Create a new object identifier for the given type.
    pub fn new<T: 'static>() -> Self {
        let type_id = TypeId::of::<T>();
        let type_name = type_name::<T>();

        Self { type_id, type_name }
    }

    /// Returns the [`TypeId`] of this tag.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the type name of this tag.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }
}

impl Object {
    /// Create a new object from the provided value.
    // TODO: We probably need to ensure that `T: Send + Sync`.
    pub fn new<T: 'static>(value: T) -> Self {
        let tag = Tag::new::<T>();
        let ptr = Box::into_raw(Box::new(value)) as *mut ();
        let drop: fn(*mut ()) = v_drop::<T>;

        Self { tag, ptr, drop }
    }

    /// Returns the [`TypeId`] of the actual type.
    pub fn type_id(&self) -> TypeId {
        self.tag.type_id
    }

    pub fn downcast<T: 'static>(&self) -> &T {
        let tid = TypeId::of::<T>();
        assert_eq!(tid, self.tag.type_id);
        unsafe { transmute(self.ptr) }
    }

    pub fn downcast_mut<T: 'static>(&self) -> &mut T {
        let tid = TypeId::of::<T>();
        assert_eq!(tid, self.tag.type_id);
        unsafe { transmute(self.ptr) }
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        (self.drop)(self.ptr);
    }
}

impl Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Object<{}@{:?}>", self.tag.type_name, self.ptr)
    }
}

impl Debug for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tag<{}>", self.type_name)
    }
}

// We ignore the textual representation of a tag when it comes to comparison
// or hashing a tag.

impl Hash for Tag {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.type_id.hash(state)
    }
}

impl Eq for Tag {}

impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.type_id.eq(&other.type_id)
    }
}

impl Ord for Tag {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.type_id.cmp(&other.type_id)
    }
}

impl PartialOrd for Tag {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.type_id.partial_cmp(&other.type_id)
    }
}

// A function that given a pointer drops a box allocation, by first casting the
// pointer to the raw pointer of type `T`.
fn v_drop<T: 'static>(ptr: *mut ()) {
    let ptr = ptr as *mut T;
    let boxed = unsafe { Box::from_raw(ptr) };
    drop(boxed);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn object_should_drop() {
        struct Obj {
            inner: Rc<RefCell<bool>>,
        }

        impl Drop for Obj {
            fn drop(&mut self) {
                *self.inner.borrow_mut() = true;
            }
        }

        let did_drop = Rc::new(RefCell::new(false));

        {
            let value = Obj {
                inner: did_drop.clone(),
            };

            let obj = Object::new(value);

            // Creation of the object should not case it to be dropped.
            assert!(!*did_drop.borrow());
        }

        // Once we're out of scope the object should have been dropped.
        assert!(*did_drop.borrow());
    }

    #[test]
    fn object_should_downcast() {
        type O = Vec<u8>;
        let obj = Object::new::<O>(vec![12, 13]);
        let o = obj.downcast::<O>();
        assert_eq!(o, &vec![12, 13]);
    }

    #[test]
    fn object_should_downcast_mut() {
        type O = Vec<u8>;
        let mut obj = Object::new::<O>(vec![12, 13]);
        let o = obj.downcast_mut::<O>();
        o.push(16);
        assert_eq!(o, &vec![12, 13, 16]);
    }

    #[test]
    #[should_panic]
    fn object_should_not_downcast_wrong_type() {
        type O = Vec<u8>;
        let obj = Object::new::<O>(vec![12, 13]);
        obj.downcast::<u8>();
    }

    #[test]
    #[should_panic]
    fn object_should_not_downcast_mut_wrong_type() {
        type O = Vec<u8>;
        let obj = Object::new::<O>(vec![12, 13]);
        obj.downcast_mut::<u8>();
    }

    #[test]
    fn object_debug() {
        type O = Vec<u8>;
        let obj = Object::new::<O>(vec![12, 13]);
        let val = format!("{:?}", obj);
        assert!(
            val.starts_with("Object<alloc::vec::Vec<u8>@"),
            "{val} is invalid debug output."
        );
    }

    #[test]
    fn tag_should_work() {
        struct A;
        struct B;
        assert_eq!(Tag::new::<B>(), Tag::new::<B>(),);
        assert_ne!(Tag::new::<A>(), Tag::new::<B>(),);
    }

    #[test]
    fn tag_debug() {
        let tag = Tag::new::<u8>();
        assert_eq!(format!("{:?}", tag), "Tag<u8>");
    }
}
