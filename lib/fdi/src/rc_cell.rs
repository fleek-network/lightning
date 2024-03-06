use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

/// A ref-cell that allows the references to outlive the cell itself.
pub struct RcCell<T> {
    cell: Cell<T>,
}

struct Cell<T>(NonNull<UnsafeCell<Inner<T>>>);

struct Inner<T> {
    mutable: bool,
    cell: bool,
    borrowed: usize,
    value: T,
}

pub struct Ref<T> {
    cell: Cell<T>,
}

pub struct RefMut<T> {
    cell: Cell<T>,
}

impl<T> RcCell<T> {
    pub fn new(value: T) -> Self {
        let inner = Box::into_raw(Box::new(UnsafeCell::new(Inner {
            mutable: false,
            cell: true,
            borrowed: 0,
            value,
        })));

        RcCell {
            cell: Cell(NonNull::new(inner).unwrap()),
        }
    }

    pub fn borrow(&self) -> Ref<T> {
        self.cell.ensure_not_mut();
        Ref::new(Cell(self.cell.0))
    }

    pub fn borrow_mut(&self) -> RefMut<T> {
        self.cell.ensure_not_mut();
        self.cell.ensure_not_borrowed();
        RefMut::new(Cell(self.cell.0))
    }
}

impl<T> Cell<T> {
    #[inline(always)]
    fn ensure_not_mut(&self) {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &*unsafe_cell.get() };
        if inner.mutable {
            panic!("Could not borrow RcCell: mutable borrow already exists.");
        }
    }

    #[inline(always)]
    fn ensure_not_borrowed(&self) {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &*unsafe_cell.get() };
        if inner.borrowed > 0 {
            panic!("Could not borrow RcCell: borrow already exists.");
        }
    }

    #[inline(always)]
    fn mark_mut_borrow(&self) {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &mut *unsafe_cell.get() };
        inner.mutable = true;
    }

    #[inline(always)]
    fn unmark_mut_borrow(&self) {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &mut *unsafe_cell.get() };
        inner.mutable = false;
    }

    #[inline(always)]
    fn unmark_cell(&self) {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &mut *unsafe_cell.get() };
        inner.cell = false;
    }

    #[inline(always)]
    fn add_borrow(&self) {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &mut *unsafe_cell.get() };
        inner.borrowed += 1;
    }

    #[inline(always)]
    fn remove_borrow(&self) {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &mut *unsafe_cell.get() };
        inner.borrowed -= 1;
    }

    #[inline(always)]
    unsafe fn get(&self) -> &T {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &*unsafe_cell.get() };
        &inner.value
    }

    #[inline(always)]
    #[allow(clippy::mut_from_ref)]
    unsafe fn get_mut(&self) -> &mut T {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &mut *unsafe_cell.get() };
        &mut inner.value
    }
}

impl<T> Ref<T> {
    #[inline(always)]
    fn new(cell: Cell<T>) -> Self {
        cell.add_borrow();
        Self { cell }
    }
}

impl<T> RefMut<T> {
    #[inline(always)]
    fn new(cell: Cell<T>) -> Self {
        cell.mark_mut_borrow();
        Self { cell }
    }
}

impl<T> Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.cell.get() }
    }
}

impl<T> Deref for RefMut<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.cell.get() }
    }
}

impl<T> DerefMut for RefMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.cell.get_mut() }
    }
}

impl<T> Drop for Ref<T> {
    fn drop(&mut self) {
        self.cell.remove_borrow();
    }
}

impl<T> Drop for RefMut<T> {
    fn drop(&mut self) {
        self.cell.unmark_mut_borrow();
    }
}

impl<T> Drop for RcCell<T> {
    fn drop(&mut self) {
        self.cell.unmark_cell();
    }
}

impl<T> Drop for Cell<T> {
    fn drop(&mut self) {
        let unsafe_cell = unsafe { self.0.as_ref() };
        let inner = unsafe { &*unsafe_cell.get() };
        if !inner.mutable && inner.borrowed == 0 && !inner.cell {
            let ptr = self.0.as_ptr();
            let b = unsafe { Box::from_raw(ptr) };
            drop(b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RcCell;

    #[test]
    fn borrow_borrow() {
        let obj = RcCell::new(String::from("Hello"));
        let a = obj.borrow();
        let b = obj.borrow();
        drop((a, b));
    }

    #[test]
    fn borrow_drop_borrow_mut() {
        let obj = RcCell::new(String::from("Hello"));
        let a = obj.borrow();
        drop(a);
        let b = obj.borrow_mut();
        drop(b);
    }

    #[test]
    fn borrow_mut_drop_borrow_mut() {
        let obj = RcCell::new(String::from("Hello"));
        let a = obj.borrow_mut();
        drop(a);
        let b = obj.borrow_mut();
        drop(b);
    }

    #[test]
    fn borrow_mut_drop_borrow() {
        let obj = RcCell::new(String::from("Hello"));
        let a = obj.borrow_mut();
        drop(a);
        let b = obj.borrow();
        drop(b);
    }

    #[test]
    #[should_panic(expected = "Could not borrow RcCell: borrow already exists.")]
    fn borrow_borrow_mut() {
        let obj = RcCell::new(String::from("Hello"));
        let a = obj.borrow();
        let b = obj.borrow_mut();
        drop((a, b));
    }

    #[test]
    #[should_panic(expected = "Could not borrow RcCell: mutable borrow already exists")]
    fn borrow_mut_borrow_mut() {
        let obj = RcCell::new(String::from("Hello"));
        let a = obj.borrow_mut();
        let b = obj.borrow_mut();
        drop((a, b));
    }

    #[test]
    fn borrow_mut_obj_drop() {
        let obj = RcCell::new(String::from("Hello"));
        let a = obj.borrow_mut();
        drop(obj);
        assert_eq!(*a, "Hello");
    }

    #[test]
    fn borrow_obj_drop() {
        let obj = RcCell::new(String::from("Hello"));
        let a = obj.borrow();
        drop(obj);
        assert_eq!(*a, "Hello");
    }
}
