range-lock - Multithread range lock for Vec
===========================================

`https://bues.ch/ <https://bues.ch/>`_

Provides multi-threaded access to a single Vec<T> instance. Any thread can atomically request access to a slice of the Vec. Such access is granted, if no other thread is simultaneously holding the right to access an overlapping slice.

Usage
=====

Add this to your Cargo.toml:

.. code:: toml

    [dependencies]
    range-lock = "0.1"

Example usage:

.. code:: rust

    extern crate range_lock;
    use range_lock::RangeLock;
    use std::sync::{Arc, Barrier};
    use std::thread;

    // The data that will simultaneously be accessed from the threads.
    let data = vec![10, 11, 12, 13];

    // Embed the data in a RangeLock
    // and clone atomic references to it for the threads.
    let data_lock0 = Arc::new(RangeLock::new(data));
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
    let data = Arc::try_unwrap(data_lock2).expect("Arc unwrap failed").into_inner();

    // Check the data that has been modified by the threads.
    assert_eq!(data, vec![100, 11, 200, 13]);


TODO
====

The API of range-lock is currently very simple and only provides the bare minimum.

The following new features might be candidates for future releases:

* Optimize the range overlap search algorithm.
* Sleeping lock, in case of lock contention.
* Add support for arrays.
* Add support for non-Vec/array iterables?


License
=======

Copyright (c) 2021 Michael Buesch <m@bues.ch>

Licensed under the Apache License version 2.0 or the MIT license, at your option.
