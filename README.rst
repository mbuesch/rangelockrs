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

	//TODO


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
