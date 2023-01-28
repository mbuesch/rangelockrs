// -*- coding: utf-8 -*-
//
// Copyright 2021-2022 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

extern crate range_lock;
use range_lock::VecRangeLock;
use std::sync::{Arc, Barrier};
use std::thread;

#[test]
fn test_rangelock() {
    // The data that will simultaneously be accessed from the threads.
    let data = vec![10, 11, 12, 13];

    // Embed the data in a VecRangeLock
    // and clone atomic references to it for the threads.
    let data_lock0 = Arc::new(VecRangeLock::new(data));
    let data_lock1 = Arc::clone(&data_lock0);
    let data_lock2 = Arc::clone(&data_lock0);

    // Thread barrier, only for demonstration purposes.
    let barrier0 = Arc::new(Barrier::new(2));
    let barrier1 = Arc::clone(&barrier0);

    // Spawn first thread.
    let thread0 = thread::spawn(move || {
        {
            let mut guard = data_lock0.try_lock(0..2).expect("T0: Failed to lock 0..2");
            guard[0] = 100; // Write to data[0]
        }
        barrier0.wait(); // Synchronize with second thread.
        {
            let guard = data_lock0.try_lock(2..4).expect("T0: Failed to lock 2..4");
            assert_eq!(guard[0], 200); // Read from data[2]
        }
    });

    // Spawn second thread.
    let thread1 = thread::spawn(move || {
        {
            let mut guard = data_lock1.try_lock(2..4).expect("T1: Failed to lock 2..4");
            guard[0] = 200; // Write to data[2]
        }
        barrier1.wait(); // Synchronize with first thread.
        {
            let guard = data_lock1.try_lock(0..2).expect("T1: Failed to lock 0..2");
            assert_eq!(guard[0], 100); // Read from data[0]
        }
    });

    // Wait for both threads to finish and check result.
    thread0.join().expect("Thread 0 failed");
    thread1.join().expect("Thread 1 failed");

    // Unwrap the data from the lock.
    let data = Arc::try_unwrap(data_lock2)
        .expect("Arc unwrap failed")
        .into_inner();

    // Check the data that has been modified by the threads.
    assert_eq!(data, vec![100, 11, 200, 13]);
}

#[test]
#[should_panic(expected = "T1: Failed to lock 1..4")]
fn test_conflict() {
    let data = vec![10, 11, 12, 13];

    let data_lock0 = Arc::new(VecRangeLock::new(data));
    let data_lock1 = Arc::clone(&data_lock0);

    let barrier0 = Arc::new(Barrier::new(2));
    let barrier1 = Arc::clone(&barrier0);

    let thread0 = thread::spawn(move || {
        let mut _guard = data_lock0.try_lock(0..2).expect("T0: Failed to lock 0..2");
        barrier0.wait();
        // try_lock() conflict happens in second thread.
        barrier0.wait();
    });

    let thread1 = thread::spawn(move || {
        barrier1.wait();
        // thread0 holds lock to 0..2, which conflicts with 1..4.
        if data_lock1.try_lock(1..4).is_err() {
            barrier1.wait();
            panic!("T1: Failed to lock 1..4");
        }
        barrier1.wait();
    });

    if let Err(e) = thread0.join() {
        panic!("Thread 0 failed: {:?}", e.downcast_ref::<&str>());
    }
    if let Err(e) = thread1.join() {
        panic!("Thread 1 failed: {:?}", e.downcast_ref::<&str>());
    }
}

// vim: ts=4 sw=4 expandtab
