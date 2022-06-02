//! Synchronized Cell
//!
//! Main principles:
//!   1. if you change state, and it's fine, you reverse it on drop()
//!   2. if you found a problem, still undo your change, and then panic()

#[cfg(loom)]
use loom as mystd;
#[cfg(not(loom))]
use std as mystd;

use mystd::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};
use std::{mem, ops};

const WRITE_BIT: usize = 1 << (mem::size_of::<usize>() * 8 - 1);

/// A shared reference to `SynCell` data.
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

/// A mutable reference to `SynCell` data.
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

/// A Sync cell. Stores a value of type `T` and allows
/// to access it behind a reference. `SynCell` follows Rust borrowing
/// rules but checks them at run time as opposed to compile time.
pub struct SynCell<T> {
    state: AtomicUsize,
    value: UnsafeCell<T>,
}

unsafe impl<T> Sync for SynCell<T> {}

impl<T> SynCell<T> {
    /// Create a new cell.
    pub fn new(value: T) -> Self {
        Self {
            state: AtomicUsize::new(0),
            value: UnsafeCell::new(value),
        }
    }

    /// Convert into the value.
    pub fn into_inner(self) -> T {
        debug_assert_eq!(self.state.load(Ordering::Acquire), 0);
        self.value.into_inner()
    }

    /// Get a direct mutable reference to the data.
    pub fn get_mut(&mut self) -> &mut T {
        debug_assert_eq!(self.state.load(Ordering::Acquire), 0);
        self.value.get_mut()
    }

    /// Borrow immutably (can be shared).
    ///
    /// Panics if the value is already borrowed mutably.
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

    /// Borrow mutably (exclusive).
    ///
    /// Panics if the value is already borrowed in any way.
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

#[test]
fn valid() {
    let sc = SynCell::new(0u8);
    {
        let mut bw = sc.borrow_mut();
        *bw += 1;
    }
    {
        let b1 = sc.borrow();
        let b2 = sc.borrow();
        assert_eq!(*b1 + *b2, 2);
    }
}

#[test]
#[should_panic]
fn bad_write_write() {
    let sc = SynCell::new(0u8);
    let _b1 = sc.borrow_mut();
    let _b2 = sc.borrow_mut();
}

#[test]
#[should_panic]
fn bad_read_write() {
    let sc = SynCell::new(0u8);
    let _b1 = sc.borrow();
    let _b2 = sc.borrow_mut();
}

#[test]
#[should_panic]
fn bad_write_read() {
    let sc = SynCell::new(0u8);
    let _b1 = sc.borrow_mut();
    let _b2 = sc.borrow();
}

#[test]
fn fight() {
    use mystd::{
        sync::{Arc, RwLock},
        thread,
    };
    const NUM_THREADS: usize = 3;
    const NUM_LOCKS: usize = if cfg!(miri) { 100 } else { 10000 };
    // Since `SynCell` is inside `RwLock`, it's guaranteed
    // that all the access is rightful, and no panic is expected.
    let value = Arc::new(RwLock::new(SynCell::new(0usize)));
    let sum = Arc::new(AtomicUsize::new(0));
    let join_handles = (0..NUM_THREADS).map(|i| {
        let sum = Arc::clone(&sum);
        let value = Arc::clone(&value);
        thread::spawn(move || {
            for j in 0..NUM_LOCKS {
                if (i + j) % NUM_THREADS == 0 {
                    let sc = value.write().unwrap();
                    *sc.borrow_mut() += 1;
                } else {
                    let sc = value.read().unwrap();
                    let v = *sc.borrow();
                    sum.fetch_add(v, Ordering::Relaxed);
                }
            }
        })
    });
    for jh in join_handles {
        jh.join().unwrap();
    }
}
