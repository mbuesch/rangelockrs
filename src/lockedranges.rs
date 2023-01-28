// -*- coding: utf-8 -*-
//
// Copyright 2021-2023 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use std::{collections::BTreeMap, ops::Range};

#[inline]
pub fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.end > b.start && a.start < b.end
}

#[derive(Debug)]
pub struct LockedRanges {
    tree: BTreeMap<usize, usize>,
}

impl LockedRanges {
    #[inline]
    pub fn new() -> Self {
        Self {
            tree: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    #[inline]
    pub fn insert(&mut self, range: &Range<usize>) -> bool {
        // Check if this range overlaps with an existing one in the tree.
        for (begin, end) in self.tree.range(..range.end).rev() {
            if *end <= range.start {
                break;
            }
            if overlaps(&(*begin..*end), range) {
                // The range overlaps with an existing one in the tree.
                return false;
            }
        }
        // The range does not overlap with an existing one in the tree.
        // Insert it into the tree.
        self.tree.insert(range.start, range.end);
        true
    }

    #[inline]
    pub fn remove(&mut self, range: &Range<usize>) {
        let end = self.tree.remove(&range.start);
        // The caller must ensure that the removed range
        // has been passed successfully to insert() before.
        debug_assert_eq!(end.unwrap(), range.end);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_overlaps() {
        assert!(overlaps(&(0..1), &(0..1)));
        assert!(!overlaps(&(0..1), &(1..2)));
        assert!(!overlaps(&(0..1), &(1..3)));
        assert!(!overlaps(&(0..1), &(2..4)));
        assert!(!overlaps(&(0..1), &(3..5)));
        assert!(!overlaps(&(0..1), &(4..6)));
        assert!(!overlaps(&(0..1), &(5..7)));
        assert!(!overlaps(&(0..1), &(6..8)));
        assert!(!overlaps(&(0..1), &(7..9)));

        assert!(!overlaps(&(4..6), &(0..1)));
        assert!(!overlaps(&(4..6), &(1..2)));
        assert!(!overlaps(&(4..6), &(1..3)));
        assert!(!overlaps(&(4..6), &(2..4)));
        assert!(overlaps(&(4..6), &(3..5)));
        assert!(overlaps(&(4..6), &(4..6)));
        assert!(overlaps(&(4..6), &(5..7)));
        assert!(!overlaps(&(4..6), &(6..8)));
        assert!(!overlaps(&(4..6), &(7..9)));
    }

    #[test]
    fn test_lockedranges() {
        let mut lr = LockedRanges::new();
        assert!(lr.is_empty());
        assert!(lr.insert(&(10..20)));
        assert!(lr.insert(&(30..40)));
        assert!(lr.insert(&(100..200)));
        assert!(lr.insert(&(1000..2000)));
        assert!(lr.insert(&(10000..20000)));
        assert!(!lr.is_empty());

        assert!(!lr.insert(&(10..20)));
        assert!(!lr.insert(&(30..40)));
        assert!(!lr.insert(&(100..200)));
        assert!(!lr.insert(&(1000..2000)));
        assert!(!lr.insert(&(10000..20000)));

        assert!(!lr.insert(&(9..11)));
        assert!(!lr.insert(&(39..41)));
        assert!(!lr.insert(&(100..101)));
        assert!(!lr.insert(&(1999..2000)));
        assert!(!lr.insert(&(15000..16000)));

        lr.remove(&(100..200));

        assert!(!lr.insert(&(9..11)));
        assert!(!lr.insert(&(39..41)));
        assert!(lr.insert(&(100..101)));
        assert!(!lr.insert(&(1999..2000)));
        assert!(!lr.insert(&(15000..16000)));

        lr.remove(&(10..20));
        lr.remove(&(30..40));
        lr.remove(&(100..101));
        lr.remove(&(1000..2000));
        lr.remove(&(10000..20000));
        assert!(lr.is_empty());
    }
}

// vim: ts=4 sw=4 expandtab
