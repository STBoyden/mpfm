// File name: utils.rs
// Author: Samuel Boyden
// Date created: 2025-08-10
// Date modified: 2025-08-27
// ------
// Copyright (c) 2025 Samuel Boyden. All rights reserved.

use pretty_duration::pretty_duration as _pretty_duration;
use std::{
    ops::Deref,
    sync::{Arc, RwLock},
    time,
};

pub(crate) fn pretty_duration(duration: &time::Duration) -> String {
    return _pretty_duration(duration, None);
}

#[derive(Debug, Clone)]
pub struct ArcRwLock<T>(Arc<RwLock<T>>);

impl<T> ArcRwLock<T> {
    pub fn new(value: T) -> Self { return Self(Arc::new(RwLock::new(value))); }
}

impl<T> Deref for ArcRwLock<T> {
    type Target = Arc<RwLock<T>>;
    fn deref(&self) -> &Self::Target { return &self.0 }
}
