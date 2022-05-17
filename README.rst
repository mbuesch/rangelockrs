range-lock - Multithread range lock for Vec
===========================================

`https://bues.ch/ <https://bues.ch/>`_

`https://bues.ch/cgit/rangelockrs.git <https://bues.ch/cgit/rangelockrs.git>`_

This crate provides locks/mutexes for multi-threaded access to a single Vec<T> instance.

Any thread can request exclusive access to a slice of the Vec.
Such access is granted, if no other thread is simultaneously holding the permission to access an overlapping slice.


Usage
=====

Add this to your Cargo.toml:

.. code:: toml

    [dependencies]
    range-lock = "0.2"


VecRangeLock example usage
--------------------------

General purpose VecRangeLock:

.. code:: rust

    use range_lock::VecRangeLock;
    use std::sync::Arc;
    use std::thread;

    let lock = Arc::new(VecRangeLock::new(vec![1, 2, 3, 4, 5]));

    thread::spawn(move || {
        let mut guard = lock.try_lock(2..4).expect("Failed to lock range 2..4");
        assert_eq!(guard[0], 3);
        guard[0] = 10;
    });


RepVecRangeLock example usage
-----------------------------

The RepVecRangeLock is a restricted range lock, that provides access to interleaved patterns of slices to the threads.

Locking a RepVecRangeLock is more lightweight than locking a VecRangeLock.
The threads can not freely choose slice ranges, but only choose a repeating slice pattern by specifying a pattern offset.

Please see the example below.

.. code:: rust

    use range_lock::RepVecRangeLock;
    use std::sync::Arc;
    use std::thread;

    let data = vec![1, 2, 3,  // <- cycle 0
                    4, 5, 6]; // <- cycle 1
    //              ^  ^  ^
    //              |  |  |
    //              |  |  offset-2
    //       offset-0  offset-1

    let lock = Arc::new(RepVecRangeLock::new(data,
                                             1,    // slice_len: Each slice has 1 element.
                                             3));  // cycle_len: Each cycle has 3 slices (offsets).
    thread::spawn(move || {
        // Lock slice offset 1:
        let mut guard = lock.try_lock(1).expect("Failed to lock offset.");

        assert_eq!(guard[0][0], 2);     // Cycle 0, Slice element 0
        assert_eq!(guard[1][0], 5);     // Cycle 1, Slice element 0

        guard[0][0] = 20;               // Cycle 0, Slice element 0
        guard[1][0] = 50;               // Cycle 1, Slice element 0
    });


TODOs for future releases
=========================

The following new features might be candidates for future releases:

* Optimize the range overlap search algorithm.
* Sleeping lock, in case of lock contention.
* Add support for arrays.


License
=======

Copyright (c) 2021-2022 Michael Buesch <m@bues.ch>

Licensed under the Apache License version 2.0 or the MIT license, at your option.
