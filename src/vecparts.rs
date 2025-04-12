// -*- coding: utf-8 -*-
//
// Copyright 2021-2025 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use std::{mem::ManuallyDrop, ptr::NonNull};

/// A `Vec<T>` deconstructed into its parts.
pub(crate) struct VecParts<T> {
    ptr: NonNull<T>,
    len: usize,
    cap: usize,
}

/// SAFETY: `VecParts<T>` is `Send` if `Vec<T>` is also `Send`.
unsafe impl<T> Send for VecParts<T> where Vec<T>: Send {}

impl<T> VecParts<T> {
    #[inline]
    pub fn ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T> From<Vec<T>> for VecParts<T> {
    #[inline]
    fn from(v: Vec<T>) -> Self {
        // Dropping will be handled by us.
        // Suppress drop from Vec by wrapping in ManuallyDrop.
        let mut v = ManuallyDrop::new(v);

        // SAFETY: Vec never returns a null pointer.
        let ptr = unsafe { NonNull::new_unchecked(v.as_mut_ptr()) };

        Self {
            ptr,
            len: v.len(),
            cap: v.capacity(),
        }
    }
}

impl<T> From<VecParts<T>> for Vec<T> {
    #[inline]
    fn from(p: VecParts<T>) -> Self {
        // The returned Vec does take care of dropping.
        // Avoid the call to our Drop handler by wrapping in ManuallyDrop.
        let p = ManuallyDrop::new(p);

        // SAFETY: This is a valid Vec and it hasn't been dropped, yet.
        unsafe { Vec::from_raw_parts(p.ptr.as_ptr(), p.len, p.cap) }
    }
}

impl<T> Drop for VecParts<T> {
    #[inline]
    fn drop(&mut self) {
        // Drop the Vec<T> properly.
        // SAFETY: This is a valid Vec and it hasn't been dropped, yet.
        unsafe {
            drop(Vec::from_raw_parts(self.ptr.as_ptr(), self.len, self.cap));
        }
    }
}

// vim: ts=4 sw=4 expandtab
