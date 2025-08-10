// File name: utils.rs
// Author: Samuel Boyden
// Date created: 2025-08-10
// Date modified: 2025-08-10
// ------
// Copyright (c) 2025 Samuel Boyden. All rights reserved.

use pretty_duration::pretty_duration as _pretty_duration;
use std::time;

pub(crate) fn pretty_duration(duration: &time::Duration) -> String {
    _pretty_duration(duration, None)
}
