// -*- coding: utf-8 -*-
//
// Copyright 2021-2023 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

use range_lock::RepVecRangeLock;
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn test_rangelock() {
    // The data that will simultaneously be accessed from the threads.
    let data = vec![10, 11, 12, 13, 20, 21, 22, 23, 30, 31, 32, 33];

    // Embed the data in a VecRangeLock
    // and clone atomic references to it for the threads.
    let data_lock0 = Arc::new(RepVecRangeLock::new(data, 1, 4));
    let data_lock1 = Arc::clone(&data_lock0);
    let data_lock2 = Arc::clone(&data_lock0);

    // Thread barrier, only for demonstration purposes.
    let barrier0 = Arc::new(Barrier::new(2));
    let barrier1 = Arc::clone(&barrier0);

    thread::scope(|s| {
        // Spawn first thread.
        s.spawn(move || {
            {
                let mut guard = data_lock0
                    .try_lock(0)
                    .expect("T0: Failed to lock offset 0.");
                guard[0][0] = 100; // Write to data[0]
                guard[1][0] = 200; // Write to data[4]
            }
            barrier0.wait(); // Synchronize with second thread.
            {
                let guard = data_lock0
                    .try_lock(1)
                    .expect("T0: Failed to lock offset 1.");
                assert_eq!(guard[0][0], 1000); // Read from data[1]
                assert_eq!(guard[1][0], 2000); // Read from data[5]
            }
        });

        // Spawn second thread.
        s.spawn(move || {
            {
                let mut guard = data_lock1
                    .try_lock(1)
                    .expect("T1: Failed to lock offset 1.");
                guard[0][0] = 1000; // Write to data[1]
                guard[1][0] = 2000; // Write to data[5]
            }
            barrier1.wait(); // Synchronize with first thread.
            {
                let guard = data_lock1
                    .try_lock(0)
                    .expect("T1: Failed to lock offset 0.");
                assert_eq!(guard[0][0], 100); // Read from data[0]
                assert_eq!(guard[1][0], 200); // Read from data[5]
            }
        });
    });

    // Unwrap the data from the lock.
    let data = Arc::try_unwrap(data_lock2)
        .expect("Arc unwrap failed")
        .into_inner();

    // Check the data that has been modified by the threads.
    assert_eq!(
        data,
        vec![100, 1000, 12, 13, 200, 2000, 22, 23, 30, 31, 32, 33]
    );
}

#[test]
fn test_conflict() {
    let data = vec![10, 11, 12, 13, 20, 21, 22, 23, 30, 31, 32, 33];

    let data_lock0 = Arc::new(RepVecRangeLock::new(data, 1, 4));
    let data_lock1 = Arc::clone(&data_lock0);

    let barrier0 = Arc::new(Barrier::new(2));
    let barrier1 = Arc::clone(&barrier0);

    thread::scope(|s| {
        s.spawn(move || {
            let mut _guard = data_lock0.try_lock(0).expect("T0: Failed to offset 0.");
            barrier0.wait();
            // try_lock() conflict happens in second thread.
            barrier0.wait();
        });

        s.spawn(move || {
            barrier1.wait();
            // thread0 holds lock offset 0.
            assert!(data_lock1.try_lock(0).is_err());
            barrier1.wait();
        });
    });
}

// vim: ts=4 sw=4 expandtab
