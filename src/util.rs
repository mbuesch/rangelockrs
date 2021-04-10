// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use std::{
    collections::HashSet,
    ops::{
        Bound,
        Range,
        RangeBounds,
    },
};

#[inline]
pub fn get_bounds(range: &impl RangeBounds<usize>, length: usize) -> (usize, usize) {
    let start = match range.start_bound() {
        Bound::Included(x) => *x,
        Bound::Excluded(_) =>
            panic!("get_bounds: Start bound must be inclusive or unbounded."),
        Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        Bound::Included(x) => {
            assert!(*x < usize::MAX);
            *x + 1 // to excluded
        },
        Bound::Excluded(x) => *x,
        Bound::Unbounded => length,
    };
    (start, end)
}

pub fn overlaps_any(ranges: &HashSet<Range<usize>>,
                    range:  &impl RangeBounds<usize>) -> bool {
    let (start, end) = get_bounds(range, usize::MAX);
    for r in ranges {
        if end > r.start && start < r.end {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlaps_any() {
        let mut a = HashSet::new();
        a.insert(0..1);
        a.insert(4..6);
        assert!(overlaps_any(&a, &(0..1)));
        assert!(!overlaps_any(&a, &(1..2)));
        assert!(!overlaps_any(&a, &(1..3)));
        assert!(!overlaps_any(&a, &(2..4)));
        assert!(overlaps_any(&a, &(3..5)));
        assert!(overlaps_any(&a, &(4..6)));
        assert!(overlaps_any(&a, &(5..7)));
        assert!(!overlaps_any(&a, &(6..8)));
        assert!(!overlaps_any(&a, &(7..9)));
    }
}

// vim: ts=4 sw=4 expandtab
