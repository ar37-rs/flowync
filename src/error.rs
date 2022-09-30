use core::fmt;
use std::{
    error::Error,
    fmt::{Debug, Formatter},
};

/// Cause of the `Flower` error
pub enum Cause {
    /// What the error message exactly supposed to be? who knows, let's guess.
    Suppose(String),
    /// Usually caused by runtime errror and things such unwrapping an error or stuff.
    Panicked(String),
}

impl Debug for Cause {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Suppose(r) => f.debug_tuple("Suppose").field(r).finish(),
            Self::Panicked(p) => f.debug_tuple("Panicked").field(p).finish(),
        }
    }
}

#[cfg(feature = "compact")]
/// Cause of the `CompactFlower` error
pub enum Compact<T> {
    /// What the error message exactly supposed to be?
    Suppose(T),
    /// Usually caused by runtime errror and things such unwrapping an error or stuff.
    Panicked(String),
}

#[cfg(feature = "compact")]
impl<T: Debug> Debug for Compact<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Suppose(r) => f.debug_tuple("Suppose").field(r).finish(),
            Self::Panicked(p) => f.debug_tuple("Panicked").field(p).finish(),
        }
    }
}

pub type IOError = Box<dyn Error>;
/// Runtime error, an alternative alias to avoid conflict with other crate type
pub type RtError = IOError;
