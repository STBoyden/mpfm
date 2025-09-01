use std::{
    ops::Deref,
    sync::{Arc, RwLock},
};

/// Simple wrapper type for [`Arc<RwLock<T>>`]. Implements [`Deref`] so for convenience of use.
#[derive(Debug, Clone)]
pub struct ArcRwLock<T>(Arc<RwLock<T>>);

impl<T> ArcRwLock<T> {
    pub fn new(value: T) -> Self { return Self(Arc::new(RwLock::new(value))); }
}

impl<T> Deref for ArcRwLock<T> {
    type Target = Arc<RwLock<T>>;
    fn deref(&self) -> &Self::Target { return &self.0 }
}
