# syncell
[![Crates.io](https://img.shields.io/crates/v/syncell.svg?label=syncell)](https://crates.io/crates/syncell)
[![Docs.rs](https://docs.rs/syncell/badge.svg)](https://docs.rs/syncell)
[![Build Status](https://github.com/kvark/syncell/workflows/check/badge.svg)](https://github.com/kvark/syncell/actions)
[![CodeCov.io](https://codecov.io/gh/kvark/syncell/branch/main/graph/badge.svg)](https://codecov.io/gh/kvark/syncell)

Just a `Sync` alternative to `std::cell::RefCell`. Useful when you have a value to share between tasks/closures,
and you already know or guarantee that the access to the value is safe (for example, via [choir task](https://github.com/kvark/choir) dependencies).

The cost of borrowing is a single atomic operation, and it's riguriously checked by both [Loom](https://github.com/tokio-rs/loom) and [Miri](https://github.com/rust-lang/miri).

## Motivation

Rust already has tools for sharing values between threads, but it strikes me that they are all rather involved and complicated:

> Shareable mutable containers exist to permit mutability in a controlled manner, even in the presence of aliasing. Both `Cell<T>` and `RefCell<T>` allow doing this in a single-threaded way. However, neither `Cell<T>` nor `RefCell<T>` are thread safe (they do not implement `Sync`). If you need to do aliasing and mutation between multiple threads it is possible to use `Mutex<T>`, `RwLock<T>` or atomic types.

This paragraph from [std::cell](https://doc.rust-lang.org/stable/std/cell/index.html) documentation proposes to use lock-based primitives as an alternative in `Sync` world. But what if you just need sharing without locking, and still want safety? `SynCell` comes to help.
