// -*- coding: utf-8 -*-
//
// Copyright 2021 Michael Büsch <m@bues.ch>
//
// Licensed under the Apache License version 2.0
// or the MIT license, at your option.
// SPDX-License-Identifier: Apache-2.0 OR MIT
//

mod rangelock;
mod reprangelock;
mod util;

pub use rangelock::{
    VecRangeLock,
    VecRangeLockGuard,
};

pub use reprangelock::{
    RepVecRangeLock,
    RepVecRangeLockGuard,
};

// vim: ts=4 sw=4 expandtab
