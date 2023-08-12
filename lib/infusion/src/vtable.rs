//! This module contains the implementation of our virtual call tables and dynamic objects,
//! the [`VTable`] a collection member, and contains the callback to the implemented functions
//! that we care about.
//!
//! Both [`VTable`] and [`Tag`] are labeled for humans, they have the trait name and the type
//! name. The type name is generated using the [`type_name`] method from the standard library.
//!
//! But due to limitations, we explicitly pass the name of a trait as `&'static str`, this makes
//! this module not really portable for other use cases and is very tuned for this library.
//!
//! However, the [`Object`] type is quite general purpose and is a very simple implementation of
//! a dynamic object (without dynamic dispatch). An object is only labeled with the named of the
//! concrete type and not the trait name.

use std::{
    any::{type_name, TypeId},
    fmt::Debug,
    hash::Hash,
    mem::transmute,
};

use crate::container::{Container, DependencyGraphVisitor};

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

/// The interned id of an object.
#[derive(Copy, Clone)]
pub struct Tag {
    /// The unique identifier for the object, which is the pointer to the `dependencies` function
    /// casted to a usize.
    id: usize,
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
        let id = (self.dependencies as *const u8) as usize;

        Tag {
            id,
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
    pub fn new<T: 'static>(trait_name: &'static str, cb: fn(&mut DependencyGraphVisitor)) -> Self {
        let id = (cb as *const u8) as usize;
        let type_name = type_name::<T>();

        Self {
            id,
            trait_name,
            type_name,
        }
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

impl Hash for Tag {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Eq for Tag {}

impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Ord for Tag {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for Tag {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
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
    use std::{cell::RefCell, rc::Rc};

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

        assert_eq!(
            Tag::new::<A>("Trait1", <A as Trait1>::dependencies),
            Tag::new::<A>("lib::Trait1", <A as Trait1>::dependencies),
        );

        assert_eq!(
            Tag::new::<A>("Trait2", <A as Trait2>::dependencies),
            Tag::new::<A>("lib::Trait2", <A as Trait2>::dependencies),
        );

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
