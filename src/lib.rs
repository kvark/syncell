//! Synchronized Cell
//!
//! Main principles:
//!   1. if you change state, and it's fine, you reverse it on drop()
//!   2. if you found a problem, still undo your change, and then panic()

use std::{
    cell::UnsafeCell,
    mem, ops,
    sync::atomic::{AtomicUsize, Ordering},
};

const WRITE_BIT: usize = 1 << (mem::size_of::<usize>() * 8 - 1);

pub struct SynRef<'a, T> {
    state: &'a AtomicUsize,
    value: &'a T,
}

impl<T> Drop for SynRef<'_, T> {
    fn drop(&mut self) {
        self.state.fetch_sub(1, Ordering::Release);
    }
}

impl<T> ops::Deref for SynRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}

pub struct SynRefMut<'a, T> {
    state: &'a AtomicUsize,
    value: &'a mut T,
}

impl<T> Drop for SynRefMut<'_, T> {
    fn drop(&mut self) {
        self.state.fetch_and(!WRITE_BIT, Ordering::Release);
    }
}

impl<T> ops::Deref for SynRefMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.value
    }
}

impl<T> ops::DerefMut for SynRefMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.value
    }
}

pub struct SynCell<T> {
    state: AtomicUsize,
    value: UnsafeCell<T>,
}

unsafe impl<T> Sync for SynCell<T> {}

impl<T> SynCell<T> {
    pub fn new(value: T) -> Self {
        Self {
            state: AtomicUsize::new(0),
            value: UnsafeCell::new(value),
        }
    }

    pub fn into_inner(self) -> T {
        debug_assert_eq!(self.state.load(Ordering::Acquire), 0);
        self.value.into_inner()
    }

    pub fn get_mut(&mut self) -> &mut T {
        debug_assert_eq!(self.state.load(Ordering::Acquire), 0);
        self.value.get_mut()
    }

    pub fn borrow(&self) -> SynRef<T> {
        let old = self.state.fetch_add(1, Ordering::AcqRel);
        if old & WRITE_BIT != 0 {
            self.state.fetch_sub(1, Ordering::Release);
            panic!("SynCell is mutably borrowed elsewhere!");
        }
        SynRef {
            state: &self.state,
            value: unsafe { &*self.value.get() },
        }
    }

    pub fn borrow_mut(&self) -> SynRefMut<T> {
        let old = self.state.fetch_or(WRITE_BIT, Ordering::AcqRel);
        if old & WRITE_BIT != 0 {
            panic!("SynCell is mutably borrowed elsewhere!");
        } else if old != 0 {
            self.state.fetch_and(!WRITE_BIT, Ordering::Release);
            panic!("SynCell is immutably borrowed elsewhere!");
        }
        SynRefMut {
            state: &self.state,
            value: unsafe { &mut *self.value.get() },
        }
    }
}
