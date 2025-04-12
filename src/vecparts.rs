// -*- coding: utf-8 -*-
//
// Copyright 2021-2025 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use std::{mem::ManuallyDrop, ptr::null_mut};

/// A `Vec<T>` deconstructed into its parts.
pub(crate) struct VecParts<T> {
    ptr: *mut T,
    len: usize,
    cap: usize,
}

/// SAFETY: `VecParts<T>` is `Send` if `Vec<T>` is also `Send`.
unsafe impl<T> Send for VecParts<T> where Vec<T>: Send {}

impl<T> VecParts<T> {
    #[inline]
    pub fn ptr(&self) -> *mut T {
        self.ptr
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T> From<Vec<T>> for VecParts<T> {
    #[inline]
    fn from(v: Vec<T>) -> Self {
        let mut v = ManuallyDrop::new(v);
        Self {
            ptr: v.as_mut_ptr(),
            len: v.len(),
            cap: v.capacity(),
        }
    }
}

impl<T> From<VecParts<T>> for Vec<T> {
    #[inline]
    fn from(mut p: VecParts<T>) -> Self {
        let ptr = p.ptr;
        let len = p.len;
        let cap = p.cap;

        p.ptr = null_mut();
        p.len = 0;
        p.cap = 0;

        unsafe { Vec::from_raw_parts(ptr, len, cap) }
    }
}

impl<T> Drop for VecParts<T> {
    #[inline]
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            let ptr = self.ptr;
            let len = self.len;
            let cap = self.cap;

            self.ptr = null_mut();
            self.len = 0;
            self.cap = 0;

            // Drop the Vec<T> properly.
            unsafe {
                drop(Vec::from_raw_parts(ptr, len, cap));
            }
        }
    }
}

// vim: ts=4 sw=4 expandtab
