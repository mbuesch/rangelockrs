// -*- coding: utf-8 -*-
//
// Copyright 2021-2023 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

//! This crate provides locks/mutexes for multi-threaded access to a single `Vec<T>` instance.
//!
//! # VecRangeLock: General purpose range lock
//!
//! This is a very basic usage example.
//! For a more complex example, please see the `VecRangeLock` struct documentation.
//!
//! ```
//! use range_lock::VecRangeLock;
//! use std::sync::Arc;
//! use std::thread;
//!
//! let lock = Arc::new(VecRangeLock::new(vec![1, 2, 3, 4, 5]));
//!
//! thread::spawn(move || {
//!     let mut guard = lock.try_lock(2..4).expect("Failed to lock range 2..4");
//!     assert_eq!(guard[0], 3);
//!     guard[0] = 10;
//! });
//! ```
//!
//! # RepVecRangeLock: Restricted interleaved pattern range lock
//!
//! This is a very basic usage example.
//! For a more complex example, please see the `RepVecRangeLock` struct documentation.
//!
//! ```
//! use range_lock::RepVecRangeLock;
//! use std::sync::Arc;
//! use std::thread;
//!
//! let data = vec![1, 2, 3,  // <- cycle 0
//!                 4, 5, 6]; // <- cycle 1
//! //              ^  ^  ^
//! //              |  |  |
//! //              |  |  offset-2
//! //       offset-0  offset-1
//!
//! let lock = Arc::new(RepVecRangeLock::new(data,
//!                                          1,    // slice_len: Each slice has 1 element.
//!                                          3));  // cycle_len: Each cycle has 3 slices (offsets).
//! thread::spawn(move || {
//!     // Lock slice offset 1:
//!     let mut guard = lock.try_lock(1).expect("Failed to lock offset.");
//!
//!     assert_eq!(guard[0][0], 2);     // Cycle 0, Slice element 0
//!     assert_eq!(guard[1][0], 5);     // Cycle 1, Slice element 0
//!
//!     guard[0][0] = 20;               // Cycle 0, Slice element 0
//!     guard[1][0] = 50;               // Cycle 1, Slice element 0
//! });
//! ```

mod lockedranges;
mod rangelock;
mod reprangelock;
mod util;

pub use rangelock::{VecRangeLock, VecRangeLockGuard};
pub use reprangelock::{RepVecRangeLock, RepVecRangeLockGuard};

// vim: ts=4 sw=4 expandtab
