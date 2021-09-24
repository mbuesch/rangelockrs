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


RangeLock example usage
-----------------------

General purpose RangeLock:

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


RepRangeLock example usage
--------------------------

The RepRangeLock is a restricted range lock, that provides interleaved patterns of slices to the threads.

Locking a RepRangeLock is more lightweight than locking a RangeLock.

Please see the example below.

.. code:: rust

    use range_lock::RepRangeLock;
    use std::sync::Arc;
    use std::thread;

    let data = vec![1, 2,  3, 4,   5,  6,   // <- cycle 0
                    7, 8,  9, 10,  11, 12]; // <- cycle 1
    //              ^--^   ^---^   ^----^
    //                |      |      |
    //          offset-0  offset-1  offset-2

    let lock = Arc::new(RepRangeLock::new(data,
                                          2,    // slice_len: Each slice has 2 elements.
                                          3));  // cycle_len: Each cycle has 3 slices (offsets).
    let lock0 = Arc::clone(&lock);
    let lock1 = Arc::clone(&lock);
    let lock2 = Arc::clone(&lock);

    let thread0 = thread::spawn(move || {
        // Lock slice offset 0:
        let mut guard = lock0.try_lock(0).expect("Failed to lock offset.");

        // Read:
        assert_eq!(guard[0][0], 1);     // Cycle 0, Slice element 0
        assert_eq!(guard[0][1], 2);     // Cycle 0, Slice element 1
        // let _ = guard[0][2];         // Would panic. Slice len is only 2.
        assert_eq!(guard[1][0], 7);     // Cycle 1, Slice element 0
        assert_eq!(guard[1][1], 8);     // Cycle 1, Slice element 1
        // let _ = guard[2][0];         // Would panic: The data vec is only 2 repeat cycles long.

        // Write:
        guard[0][0] = 10;               // Cycle 0, Slice element 0
        guard[0][1] = 20;               // Cycle 0, Slice element 1
        // guard[0][2] = 42;            // Would panic: Slice len is only 2.
        guard[1][0] = 30;               // Cycle 1, Slice element 0
        guard[1][1] = 40;               // Cycle 1, Slice element 1
        // guard[2][0] = 42;            // Would panic: The data vec is only 2 repeat cycles long.
    });

    let thread1 = thread::spawn(move || {
        // Lock slice offset 1:
        let mut guard = lock1.try_lock(1).expect("Failed to lock offset.");

        guard[0][0] = 100;              // Cycle 0, Slice element 0
        guard[0][1] = 200;              // Cycle 0, Slice element 1
        guard[1][0] = 300;              // Cycle 1, Slice element 0
        guard[1][1] = 400;              // Cycle 1, Slice element 1
    });

    let thread2 = thread::spawn(move || {
        // Lock slice offset 2:
        let mut guard = lock2.try_lock(2).expect("Failed to lock offset.");

        guard[0][0] = 1000;             // Cycle 0, Slice element 0
        guard[0][1] = 2000;             // Cycle 0, Slice element 1
        guard[1][0] = 3000;             // Cycle 1, Slice element 0
        guard[1][1] = 4000;             // Cycle 1, Slice element 1
    });

    thread0.join();
    thread1.join();
    thread2.join();

    // Get the data that has been modified by the threads.
    let data = Arc::try_unwrap(lock).expect("Thread is still using data.").into_inner();

    assert_eq!(data,
               vec![10, 20, 100, 200, 1000, 2000,
                    30, 40, 300, 400, 3000, 4000]);


TODOs for future releases
=========================

The following new features might be candidates for future releases:

* Optimize the range overlap search algorithm.
* Sleeping lock, in case of lock contention.
* Add support for arrays.
* Add support for non-Vec/array iterables?


License
=======

Copyright (c) 2021 Michael Buesch <m@bues.ch>

Licensed under the Apache License version 2.0 or the MIT license, at your option.
