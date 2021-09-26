// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use std::{
    cell::UnsafeCell,
    hint::unreachable_unchecked,
    ops::{
        Index,
        IndexMut,
    },
    sync::{
        atomic::{
            AtomicU32,
            Ordering,
        },
        TryLockError,
        TryLockResult,
    }
};

/// Interleaved multi-thread range lock for `Vec<T>`.
///
/// Each thread can lock a set of repeating slices of the data.
/// The slices are interleaved with each other
/// and the slice pattern cyclically repeats at `cycle_len` rate.
///
/// Offsets are not bound to one specific thread.
///
/// Please see the example below.
///
/// # Example
///
/// ```
/// use range_lock::RepVecRangeLock;
/// use std::sync::Arc;
/// use std::thread;
///
/// let data = vec![1, 2,  3, 4,   5,  6,   // <- cycle 0
///                 7, 8,  9, 10,  11, 12]; // <- cycle 1
/// //              ^--^   ^---^   ^----^
/// //                |      |      |
/// //          offset-0  offset-1  offset-2
///
/// let lock = Arc::new(RepVecRangeLock::new(data,
///                                       2,    // slice_len: Each slice has 2 elements.
///                                       3));  // cycle_len: Each cycle has 3 slices (offsets).
/// let lock0 = Arc::clone(&lock);
/// let lock1 = Arc::clone(&lock);
/// let lock2 = Arc::clone(&lock);
///
/// let thread0 = thread::spawn(move || {
///     // Lock slice offset 0:
///     let mut guard = lock0.try_lock(0).expect("Failed to lock offset.");
///
///     // Read:
///     assert_eq!(guard[0][0], 1);     // Cycle 0, Slice element 0
///     assert_eq!(guard[0][1], 2);     // Cycle 0, Slice element 1
///     // let _ = guard[0][2];         // Would panic. Slice len is only 2.
///     assert_eq!(guard[1][0], 7);     // Cycle 1, Slice element 0
///     assert_eq!(guard[1][1], 8);     // Cycle 1, Slice element 1
///     // let _ = guard[2][0];         // Would panic: The data vec is only 2 repeat cycles long.
///
///     // Write:
///     guard[0][0] = 10;               // Cycle 0, Slice element 0
///     guard[0][1] = 20;               // Cycle 0, Slice element 1
///     // guard[0][2] = 42;            // Would panic: Slice len is only 2.
///     guard[1][0] = 30;               // Cycle 1, Slice element 0
///     guard[1][1] = 40;               // Cycle 1, Slice element 1
///     // guard[2][0] = 42;            // Would panic: The data vec is only 2 repeat cycles long.
/// });
///
/// let thread1 = thread::spawn(move || {
///     // Lock slice offset 1:
///     let mut guard = lock1.try_lock(1).expect("Failed to lock offset.");
///
///     guard[0][0] = 100;              // Cycle 0, Slice element 0
///     guard[0][1] = 200;              // Cycle 0, Slice element 1
///     guard[1][0] = 300;              // Cycle 1, Slice element 0
///     guard[1][1] = 400;              // Cycle 1, Slice element 1
/// });
///
/// let thread2 = thread::spawn(move || {
///     // Lock slice offset 2:
///     let mut guard = lock2.try_lock(2).expect("Failed to lock offset.");
///
///     guard[0][0] = 1000;             // Cycle 0, Slice element 0
///     guard[0][1] = 2000;             // Cycle 0, Slice element 1
///     guard[1][0] = 3000;             // Cycle 1, Slice element 0
///     guard[1][1] = 4000;             // Cycle 1, Slice element 1
/// });
///
/// thread0.join();
/// thread1.join();
/// thread2.join();
///
/// // Get the data that has been modified by the threads.
/// let data = Arc::try_unwrap(lock).expect("Thread is still using data.").into_inner();
///
/// assert_eq!(data,
///            vec![10, 20, 100, 200, 1000, 2000,
///                 30, 40, 300, 400, 3000, 4000]);
/// ```
#[derive(Debug)]
pub struct RepVecRangeLock<T> {
    /// Range length, in number of data elements.
    slice_len:          usize,
    /// Cycle length, in number of slices.
    cycle_len:          usize,
    /// Cycle length, in number of data elements.
    cycle_num_elems:    usize,
    /// Bitmask of locked cycle offsets.
    locked_offsets:     Vec<AtomicU32>,
    /// The protected data.
    data:               UnsafeCell<Vec<T>>,
}

// SAFETY:
// It is safe to access RepVecRangeLock and the contained data (via RepVecRangeLockGuard)
// from multiple threads simultaneously.
// The lock ensures that access to the data is strictly serialized.
// T must be Send-able to other threads.
unsafe impl<T> Sync for RepVecRangeLock<T>
where
    T: Send
{ }

impl<'a, T> RepVecRangeLock<T> {
    /// Construct a new RepVecRangeLock.
    ///
    /// * `data`: The data Vec to protect.
    /// * `slice_len`: The length of the slices, in number of elements. Must be >0.
    /// * `cycle_len`: The length of the repeat cycle, in number of slices. Must be >0 and <=usize::MAX-31.
    pub fn new(data: Vec<T>,
               slice_len: usize,
               cycle_len: usize) -> RepVecRangeLock<T> {
        if slice_len == 0 {
            panic!("slice_len must not be 0.");
        }
        if cycle_len == 0 || cycle_len > usize::MAX - 31 {
            panic!("cycle_len out of range.");
        }

        let cycle_num_elems = match cycle_len.checked_mul(slice_len) {
            Some(x) => x,
            None => panic!("Repeat cycle overflow."),
        };

        let num = (cycle_len + 31) / 32;
        let mut locked_offsets = Vec::with_capacity(num);
        locked_offsets.resize_with(num, || AtomicU32::new(0));

        let data = UnsafeCell::new(data);

        RepVecRangeLock {
            slice_len,
            cycle_len,
            cycle_num_elems,
            locked_offsets,
            data,
        }
    }

    /// Get the length (in number of elements) of the embedded Vec.
    #[inline]
    pub fn data_len(&self) -> usize {
        // SAFETY: Multithreaded access is safe. len cannot change.
        unsafe { (*self.data.get()).len() }
    }

    /// Unwrap the VecRangeLock into the contained data.
    /// This method consumes self.
    #[inline]
    pub fn into_inner(self) -> Vec<T> {
        debug_assert!(self.locked_offsets.iter().all(|x| x.load(Ordering::Acquire) == 0));
        self.data.into_inner()
    }

    /// Try to lock the given data slice at 'cycle_offset'.
    ///
    /// * On success: Returns a `RepVecRangeLockGuard` that can be used to access the locked region.
    ///               Indexing `RepVecRangeLockGuard` yields a slice of the `data`.
    /// * On failure: Returns TryLockError::WouldBlock, if the slice is contended.
    ///               The locking attempt may be retried by the caller upon contention.
    ///               Returns TryLockError::Poisoned, if the lock is poisoned.
    #[inline]
    pub fn try_lock(&'a self, cycle_offset: usize) -> TryLockResult<RepVecRangeLockGuard<'a, T>> {
        if cycle_offset >= self.cycle_len {
            panic!("Invalid cycle_offset. It must be 0 <= cycle_offset < cycle_len.");
        }
        let idx = cycle_offset / 32;
        let mask = 1 << (cycle_offset % 32);
        // SAFETY: cycle_offset has been checked against cycle_len.
        let prev = unsafe { self.locked_offsets.get_unchecked(idx) }
            .fetch_or(mask, Ordering::AcqRel);
        if prev & mask == 0 {
            // Multiply cannot overflow due to slice_len, cycle_len and cycle_offset checks.
            let cycle_offset_slices = self.slice_len * cycle_offset;
            // Successfully acquired the lock.
            TryLockResult::Ok(RepVecRangeLockGuard::new(self, cycle_offset, cycle_offset_slices))
        } else {
            // Already locked by another thread.
            TryLockResult::Err(TryLockError::WouldBlock)
        }
    }

    /// Unlock a slice at 'cycle_offset'.
    #[inline]
    fn unlock(&self, cycle_offset: usize) {
        let idx = cycle_offset / 32;
        let mask = 1 << (cycle_offset % 32);
        // SAFETY: cycle_offset has been checked against cycle_len in try_lock().
        let prev = unsafe { self.locked_offsets.get_unchecked(idx) }
            .fetch_xor(mask, Ordering::Release);
        debug_assert!(prev & mask != 0);
    }

    /// Get an immutable slice at 'cycle' / 'cycle_offset'.
    ///
    /// # SAFETY
    ///
    /// See get_mut_slice().
    #[inline]
    unsafe fn get_slice(&self,
                        cycle_offset_slices: usize,
                        cycle: usize) -> &[T] {
        if let Some(cycle_elemidx) = self.cycle_num_elems.checked_mul(cycle) {
            if let Some(begin) = cycle_elemidx.checked_add(cycle_offset_slices) {
                if let Some(end) = begin.checked_add(self.slice_len) {
                    let dataptr = self.data.get();
                    if end <= (*dataptr).len() {
                        // SAFETY: We trust the slicing machinery of Vec to work correctly.
                        //         It must return the slice range that we requested.
                        //         Otherwise our non-overlap guarantees are gone.
                        return &(*dataptr)[begin..end];
                    }
                }
            }
        }
        panic!("RepVecRangeLock cycle index out of range.");
    }

    /// Get a mutable slice at 'cycle' / 'cycle_offset'.
    ///
    /// # SAFETY
    ///
    /// The caller must ensure that:
    /// * No overlapping slices must coexist on multiple threads.
    /// * Immutable slices to overlapping ranges may only coexist on a single thread.
    /// * Immutable and mutable slices must not coexist.
    #[inline]
    #[allow(clippy::mut_from_ref)] // Slices won't overlap. See SAFETY.
    unsafe fn get_mut_slice(&self,
                            cycle_offset_slices: usize,
                            cycle: usize) -> &mut [T] {
        let cptr = self.get_slice(cycle_offset_slices, cycle) as *const [T];
        let mut_slice = (cptr as *mut [T]).as_mut();
        // SAFETY: The pointer is never null, because it has been casted from a slice.
        mut_slice.unwrap_or_else(|| unreachable_unchecked())
    }
}

/// Lock guard variable type.
///
/// The Deref and DerefMut traits are implemented for this struct.
/// See the documentation of `RepVecRangeLock` for usage examples of `RepVecRangeLockGuard`.
#[derive(Debug)]
pub struct RepVecRangeLockGuard<'a, T> {
    lock:                   &'a RepVecRangeLock<T>,
    cycle_offset:           usize,
    cycle_offset_slices:    usize,
}

impl<'a, T> RepVecRangeLockGuard<'a, T> {
    #[inline]
    fn new(lock:                &'a RepVecRangeLock<T>,
           cycle_offset:        usize,
           cycle_offset_slices: usize) -> RepVecRangeLockGuard<'a, T> {
        RepVecRangeLockGuard {
            lock,
            cycle_offset,
            cycle_offset_slices,
        }
    }
}

impl<'a, T> Drop for RepVecRangeLockGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.unlock(self.cycle_offset);
    }
}

impl<'a, T> Index<usize> for RepVecRangeLockGuard<'a, T> {
    type Output = [T];

    #[inline]
    fn index(&self, cycle: usize) -> &Self::Output {
        // SAFETY: See index_mut().
        unsafe { self.lock.get_slice(self.cycle_offset_slices, cycle) }
    }
}

impl<'a, T> IndexMut<usize> for RepVecRangeLockGuard<'a, T> {
    #[inline]
    fn index_mut(&mut self, cycle: usize) -> &mut Self::Output {
        // SAFETY:
        // The lifetime of the slice is bounded by the lifetime of the guard.
        // The lifetime of the guard is bounded by the lifetime of the range lock.
        // The underlying data is owned by the range lock.
        // Therefore the slice cannot outlive the data.
        // The range lock ensures that no overlapping/conflicting guards
        // can be constructed.
        // The compiler ensures that the DerefMut result cannot be used,
        // if there's also an immutable Deref result.
        unsafe { self.lock.get_mut_slice(self.cycle_offset_slices, cycle) }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use super::*;

    #[test]
    #[should_panic(expected="cycle_len out of range")]
    fn test_oob_slice_len() {
        let _ = RepVecRangeLock::new(vec![0; 100], 1, 0);
    }

    #[test]
    #[should_panic(expected="cycle_len out of range")]
    fn test_oob_cycle_len1() {
        let _ = RepVecRangeLock::new(vec![0; 100], 1, usize::MAX - 30);
    }

    #[test]
    #[should_panic(expected="slice_len must not be 0")]
    fn test_oob_cycle_len0() {
        let _ = RepVecRangeLock::new(vec![0; 100], 0, 1);
    }

    #[test]
    #[should_panic(expected="cycle overflow")]
    fn test_oob_cycle_len2() {
        let _ = RepVecRangeLock::new(vec![0; 100], usize::MAX, 2);
    }

    #[test]
    #[should_panic(expected="must be 0 <= cycle_offset < cycle_len")]
    fn test_oob_lock_offset() {
        let a = RepVecRangeLock::new(vec![0; 100], 2, 10);
        let _ = a.try_lock(10);
    }

    #[test]
    #[should_panic(expected="index out of bounds")]
    fn test_base_oob_read() {
        let a = RepVecRangeLock::new(vec![0; 100], 1, 2);
        let g = a.try_lock(0).unwrap();
        let _ = g[0][1];
    }

    #[test]
    #[should_panic(expected="guard 1 panicked")]
    fn test_overlap0() {
        let a = RepVecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6], 1, 3);
        let _g0 = a.try_lock(0).expect("guard 0 panicked");
        let _g1 = a.try_lock(0).expect("guard 1 panicked");
    }

    #[test]
    #[should_panic(expected="guard 1 panicked")]
    fn test_overlap1() {
        let a = RepVecRangeLock::new(vec![1_i32, 2, 3, 4, 5, 6], 1, 3);
        let _g0 = a.try_lock(1).expect("guard 0 panicked");
        let _g1 = a.try_lock(1).expect("guard 1 panicked");
    }

    #[test]
    fn test_big_cycle() {
        let a = Arc::new(RepVecRangeLock::new(vec![1_i32; 256],
                                                2,      // slice_len
                                                128));  // cycle_len
        assert!(a.locked_offsets.len() == 4);
        {
            let _g = a.try_lock(0);
            assert!(a.locked_offsets[0].load(Ordering::Acquire) == 1);
            assert!(a.locked_offsets[1].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[2].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[3].load(Ordering::Acquire) == 0);
        }
        {
            let _g = a.try_lock(1);
            assert!(a.locked_offsets[0].load(Ordering::Acquire) == 2);
            assert!(a.locked_offsets[1].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[2].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[3].load(Ordering::Acquire) == 0);
        }
        {
            let _g = a.try_lock(32);
            assert!(a.locked_offsets[0].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[1].load(Ordering::Acquire) == 1);
            assert!(a.locked_offsets[2].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[3].load(Ordering::Acquire) == 0);
        }
        {
            let _g = a.try_lock(33);
            assert!(a.locked_offsets[0].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[1].load(Ordering::Acquire) == 2);
            assert!(a.locked_offsets[2].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[3].load(Ordering::Acquire) == 0);
        }
        {
            let _g = a.try_lock(69);
            assert!(a.locked_offsets[0].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[1].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[2].load(Ordering::Acquire) == 32);
            assert!(a.locked_offsets[3].load(Ordering::Acquire) == 0);
        }
        {
            let _g = a.try_lock(127);
            assert!(a.locked_offsets[0].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[1].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[2].load(Ordering::Acquire) == 0);
            assert!(a.locked_offsets[3].load(Ordering::Acquire) == 0x80000000);
        }
    }

    #[test]
    #[should_panic(expected="Invalid cycle_offset")]
    fn test_cycle_offset_out_of_range() {
        let a = Arc::new(RepVecRangeLock::new(vec![1_i32; 256],
                                                2,      // slice_len
                                                128));  // cycle_len
        let _g = a.try_lock(128);
    }
 
    #[test]
    fn test_thread_no_overlap() {
        let a = Arc::new(RepVecRangeLock::new(vec![1_i32, 2, 3, 4],
                                                1,      // slice_len
                                                2));    // cycle_len
        let b = Arc::clone(&a);
        let c = Arc::clone(&a);
        let ba0 = Arc::new(Barrier::new(2));
        let ba1 = Arc::clone(&ba0);
        let j0 = thread::spawn(move || {
            {
                let mut g = b.try_lock(0).unwrap();
                assert!(b.locked_offsets[0].load(Ordering::Acquire) & 1 != 0);
                assert_eq!(g[0][0], 1);
                assert_eq!(g[1][0], 3);
                g[0][0] = 10;
                g[1][0] = 30;
            }
            ba0.wait();
        });
        let j1 = thread::spawn(move || {
            {
                let g = c.try_lock(1).unwrap();
                assert!(c.locked_offsets[0].load(Ordering::Acquire) & 2 != 0);
                assert_eq!(g[0][0], 2);
                assert_eq!(g[1][0], 4);
            }
            ba1.wait();
            let g = c.try_lock(0).unwrap();
            assert_eq!(g[0][0], 10);
            assert_eq!(g[1][0], 30);
        });
        j1.join().expect("Thread 1 panicked.");
        j0.join().expect("Thread 0 panicked.");
        assert!(a.locked_offsets.iter().all(|x| x.load(Ordering::Acquire) == 0));
    }

    struct NoSyncStruct(RefCell<u32>); // No Sync auto-trait.

    #[test]
    fn test_nosync() {
        let a = Arc::new(RepVecRangeLock::new(vec![
            NoSyncStruct(RefCell::new(1)),
            NoSyncStruct(RefCell::new(2)),
            NoSyncStruct(RefCell::new(3)),
            NoSyncStruct(RefCell::new(4)),
        ],
            1,      // slice_len
            2));    // cycle_len
        let b = Arc::clone(&a);
        let c = Arc::clone(&a);
        let ba0 = Arc::new(Barrier::new(2));
        let ba1 = Arc::clone(&ba0);
        let j0 = thread::spawn(move || {
            let _g = b.try_lock(0).unwrap();
            assert!(b.locked_offsets[0].load(Ordering::Acquire) & 1 != 0);
            ba0.wait();
        });
        let j1 = thread::spawn(move || {
            let _g = c.try_lock(1).unwrap();
            assert!(c.locked_offsets[0].load(Ordering::Acquire) & 2 != 0);
            ba1.wait();
        });
        j1.join().expect("Thread 1 panicked.");
        j0.join().expect("Thread 0 panicked.");
        assert!(a.locked_offsets.iter().all(|x| x.load(Ordering::Acquire) == 0));
    }
}

// vim: ts=4 sw=4 expandtab
