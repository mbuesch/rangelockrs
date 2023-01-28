// -*- coding: utf-8 -*-
//
// Copyright 2021-2023 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use std::ops::{Bound, RangeBounds};

/// Get the `(start, end)` bounds from a `RangeBounds<usize>` trait.
/// `start` is inclusive and `end` is exclusive.
#[inline]
pub fn get_bounds(range: &impl RangeBounds<usize>, length: usize) -> (usize, usize) {
    let start = match range.start_bound() {
        Bound::Included(x) => *x,
        Bound::Excluded(_) => panic!("get_bounds: Start bound must be inclusive or unbounded."),
        Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        Bound::Included(x) => {
            assert!(*x < usize::MAX);
            *x + 1 // to excluded
        }
        Bound::Excluded(x) => *x,
        Bound::Unbounded => length,
    };
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_bounds() {
        assert_eq!(get_bounds(&(10..20), usize::MAX), (10, 20));
        assert_eq!(get_bounds(&(10..20), 0), (10, 20));
        assert_eq!(get_bounds(&(10..=20), 0), (10, 21));
        assert_eq!(get_bounds(&(..20), 0), (0, 20));
        assert_eq!(get_bounds(&(..=20), 0), (0, 21));
        assert_eq!(get_bounds(&(10..), 42), (10, 42));
        assert_eq!(get_bounds(&(..), 42), (0, 42));
    }

    #[test]
    #[should_panic(expected = "< usize::MAX")]
    fn test_get_bounds_end_panic() {
        get_bounds(&(..=usize::MAX), 0);
    }
}

// vim: ts=4 sw=4 expandtab
