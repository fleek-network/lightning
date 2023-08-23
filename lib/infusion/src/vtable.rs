//! This module contains the implementation of our virtual call tables and dynamic objects,
//! the [VTable] a collection member, and contains the callback to the implemented functions
//! that we care about.
//!
//! Both [VTable] and [Tag] are labeled for humans, they have the trait name and the type
//! name. The type name is generated using the [type_name](std::any::type_name) method from
//! the standard library.
//!
//! But due to limitations, we explicitly pass the name of a trait as `&'static str`, this makes
//! this module not really portable for other use cases and is very tuned for this library.
//!
//! However, the [Object] type is quite general purpose and is a very simple implementation of
//! a dynamic object (without dynamic dispatch). An object is only labeled with the named of the
//! concrete type and not the trait name.
//!
//!
//! [VTable]: ./struct.VTable.html
//! [Tag]: ./struct.Tag.html
//! [Object]: ./struct.Object.html

use std::any::{type_name, TypeId};
use std::fmt::Debug;
use std::hash::Hash;
use std::mem::transmute;

use crate::container::Container;
use crate::graph::DependencyGraphVisitor;

/// The description of a dependency node.
pub struct VTable {
    /// Name of the trait.
    trait_name: &'static str,
    /// Name of the type implementing this trait.
    type_name: &'static str,
    /// The id for the concrete type.
    tid: TypeId,
    /// The callback function that will collect the dependencies.
    dependencies: fn(&mut DependencyGraphVisitor),
    /// The initialization function that will initialize the object and returns it.
    init: fn(&Container) -> Result<Object, Box<dyn std::error::Error>>,
    /// The post initialization function.
    post: fn(&mut Object, &Container),
}

/// Heap allocated dynamic object.
pub struct Object {
    /// The name of the underlying type for debug statement purposes.
    type_name: &'static str,
    /// The raw pointer to the data.
    ptr: *mut (),
    /// The drop function that can drop this value.
    drop: fn(*mut ()),
    /// The standard library type identifier of the actual type that this object
    /// was constructed from.
    tid: TypeId,
}

/// The interned id of an object that is trait aware.
#[derive(Copy, Clone)]
pub struct Tag {
    /// The rust type id.
    type_id: TypeId,
    /// Name of the trait. Used as a debug label. Does not effect on equality checks and the hash.
    trait_name: &'static str,
    /// Name of the type implementing this trait. Same as above, only for human labeling.
    type_name: &'static str,
}

impl VTable {
    /// Create a new type vtable for a dependency vertex.
    pub fn new<T: 'static>(
        trait_name: &'static str,
        dependencies: fn(&mut DependencyGraphVisitor),
        init: fn(&Container) -> Result<Object, Box<dyn std::error::Error>>,
        post: fn(&mut Object, &Container),
    ) -> Self {
        let type_name = type_name::<T>();
        let tid = TypeId::of::<T>();

        VTable {
            trait_name,
            type_name,
            tid,
            dependencies,
            init,
            post,
        }
    }

    /// Returns the tag of the object for this vtable.
    pub fn tag(&self) -> Tag {
        Tag {
            type_id: self.tid,
            trait_name: self.trait_name,
            type_name: self.type_name,
        }
    }

    /// Run the dependency collector.
    pub fn dependencies(&self, visitor: &mut DependencyGraphVisitor) {
        (self.dependencies)(visitor)
    }

    /// Run the object initializer and return the constructed object.
    ///
    /// # Panics
    ///
    /// If the VTable is initialized with an `init` function that returns an object of any
    /// type other than the generic passed to the [`VTable::new`], this will cause a panic.
    pub fn init(&self, container: &Container) -> Result<Object, Box<dyn std::error::Error>> {
        let object = (self.init)(container)?;
        assert_eq!(
            object.type_id(),
            self.tid,
            "Generated object is not of type required by the VTable instance."
        );
        Ok(object)
    }

    /// Run the object post initialization function.
    ///
    /// # Panics
    ///
    /// If the object passed has a type other than that of the `VTable`.
    pub fn post(&self, object: &mut Object, container: &Container) {
        assert_eq!(
            object.type_id(),
            self.tid,
            "Provided object is not of type required by the VTable instance."
        );

        (self.post)(object, container);
    }
}

impl Tag {
    /// Create a new object identifier from a dependency visitor callback.
    ///
    /// You can also use the [`tag`](crate::tag) macro in order to generate
    /// a tag.
    pub fn new<T: 'static>(trait_name: &'static str, _cb: fn(&mut DependencyGraphVisitor)) -> Self {
        // The call back above is only a marker input to avoid misuse of a tag.

        let type_id = TypeId::of::<T>();
        let type_name = type_name::<T>();

        Self {
            type_id,
            trait_name,
            type_name,
        }
    }

    /// Returns the [`TypeId`] of this tag.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns the trait name of this tag.
    pub fn trait_name(&self) -> &'static str {
        self.trait_name
    }

    /// Returns the type name of this tag.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }
}

impl Object {
    /// Create a new object from the provided value.
    pub fn new<T: 'static>(value: T) -> Self {
        let type_name = type_name::<T>();
        let tid = TypeId::of::<T>();
        let ptr = Box::into_raw(Box::new(value)) as *mut ();
        let drop: fn(*mut ()) = v_drop::<T>;

        Self {
            type_name,
            ptr,
            drop,
            tid,
        }
    }

    /// Returns the [`TypeId`] of the actual type.
    pub fn type_id(&self) -> TypeId {
        self.tid
    }

    pub fn downcast<T: 'static>(&self) -> &T {
        let tid = TypeId::of::<T>();
        assert_eq!(tid, self.tid);
        unsafe { transmute(self.ptr) }
    }

    pub fn downcast_mut<T: 'static>(&self) -> &mut T {
        let tid = TypeId::of::<T>();
        assert_eq!(tid, self.tid);
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
        write!(f, "Object<{}@{:?}>", self.type_name, self.ptr)
    }
}

impl Debug for VTable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VTable<{} as {}>", self.type_name, self.trait_name)
    }
}

impl Debug for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tag<{} as {}>", self.type_name, self.trait_name)
    }
}

#[derive(Hash, Eq, PartialEq, Ord, PartialOrd)]
struct TagCmpHack(TypeId, &'static str);

impl From<&Tag> for TagCmpHack {
    fn from(value: &Tag) -> Self {
        Self(value.type_id, value.trait_name)
    }
}

impl Hash for Tag {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        TagCmpHack::from(self).hash(state)
    }
}

impl Eq for Tag {}

impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        TagCmpHack::from(self).eq(&TagCmpHack::from(other))
    }
}

impl Ord for Tag {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        TagCmpHack::from(self).cmp(&TagCmpHack::from(other))
    }
}

impl PartialOrd for Tag {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        TagCmpHack::from(self).partial_cmp(&TagCmpHack::from(other))
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
        trait Trait1 {
            fn dependencies(visitor: &mut DependencyGraphVisitor) {}
        }

        trait Trait2 {
            fn dependencies(visitor: &mut DependencyGraphVisitor) {}
        }

        struct A;
        struct B;
        impl Trait1 for A {};
        impl Trait1 for B {};
        impl Trait2 for A {};
        impl Trait2 for B {};

        // The equality should hold regardless of the label.

        // Maybe one day we can have this back. Until then we have to enforce tags
        // to use the same human readable name.
        //
        // assert_eq!(
        //     Tag::new::<A>("Trait1", <A as Trait1>::dependencies),
        //     Tag::new::<A>("lib::Trait1", <A as Trait1>::dependencies),
        // );
        //
        // assert_eq!(
        //     Tag::new::<A>("Trait2", <A as Trait2>::dependencies),
        //     Tag::new::<A>("lib::Trait2", <A as Trait2>::dependencies),
        // );

        assert_eq!(
            Tag::new::<B>("Trait1", <B as Trait1>::dependencies),
            Tag::new::<B>("Trait1", <B as Trait1>::dependencies),
        );

        assert_eq!(
            Tag::new::<B>("Trait2", <B as Trait2>::dependencies),
            Tag::new::<B>("Trait2", <B as Trait2>::dependencies),
        );

        assert_ne!(
            Tag::new::<A>("Trait1", <A as Trait1>::dependencies),
            Tag::new::<A>("Trait2", <A as Trait2>::dependencies),
        );

        assert_ne!(
            Tag::new::<B>("Trait1", <B as Trait1>::dependencies),
            Tag::new::<B>("Trait2", <B as Trait2>::dependencies),
        );

        assert_ne!(
            Tag::new::<A>("Trait1", <A as Trait1>::dependencies),
            Tag::new::<B>("Trait1", <B as Trait1>::dependencies),
        );

        assert_ne!(
            Tag::new::<A>("Trait2", <A as Trait2>::dependencies),
            Tag::new::<B>("Trait2", <B as Trait2>::dependencies),
        );
    }

    #[test]
    fn tag_debug() {
        trait Trait1 {
            fn dependencies(visitor: &mut DependencyGraphVisitor) {}
        }

        struct A;
        impl Trait1 for A {};
        impl Trait1 for u8 {};

        let tag = Tag::new::<u8>("Trait1", <u8 as Trait1>::dependencies);
        assert_eq!(format!("{:?}", tag), "Tag<u8 as Trait1>");

        let tag = Tag::new::<A>("Trait1", <A as Trait1>::dependencies);
        assert_eq!(
            format!("{:?}", tag),
            "Tag<infusion::vtable::tests::tag_debug::A as Trait1>"
        );
    }

    #[test]
    fn v_table_tag_should_be_correct() {
        trait Trait1 {
            fn dependencies(visitor: &mut DependencyGraphVisitor) {}
            fn init(c: &Container) -> Result<Object, Box<dyn std::error::Error>> {
                todo!()
            }
            fn post(o: &mut Object, c: &Container) {}
        }

        struct A;
        impl Trait1 for A {};

        let vtable = VTable::new::<A>(
            "Trait1",
            <A as Trait1>::dependencies,
            <A as Trait1>::init,
            <A as Trait1>::post,
        );

        assert_eq!(
            Tag::new::<A>("Trait1", <A as Trait1>::dependencies),
            vtable.tag()
        );
    }

    #[test]
    fn v_table_should_forward_method_call() {
        #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
        struct Obj {
            initialization_value: u8,
            post_called: bool,
        }

        fn dependencies(visitor: &mut DependencyGraphVisitor) {
            visitor.mark_input();
        }

        fn init(c: &Container) -> Result<Object, Box<dyn std::error::Error>> {
            Ok(Object::new(Obj {
                initialization_value: 27,
                post_called: false,
            }))
        }

        fn post(o: &mut Object, c: &Container) {
            o.downcast_mut::<Obj>().post_called = true;
        }

        let vtable = VTable::new::<Obj>("unknown", dependencies, init, post);

        // call to dependencies should work.
        {
            let mut visitor = DependencyGraphVisitor::default();
            visitor.set_current(Tag::new::<Obj>("unknown", dependencies));
            assert!(visitor.inputs.is_empty());
            vtable.dependencies(&mut visitor);
            assert_eq!(visitor.inputs.len(), 1);
        }

        // call to init should work.
        let mut container = Container::default();
        let mut obj = vtable.init(&container).unwrap();

        assert_eq!(
            obj.downcast::<Obj>(),
            &Obj {
                initialization_value: 27,
                post_called: false
            }
        );

        // call to post should work.
        vtable.post(&mut obj, &container);

        assert_eq!(
            obj.downcast::<Obj>(),
            &Obj {
                initialization_value: 27,
                post_called: true
            }
        );
    }
}
