#[cfg(feature = "use-usync")]
mod usync_mutex;
#[cfg(feature = "use-usync")]
pub use usync_mutex::{Flower, FlowerHandle};

#[cfg(not(feature = "use-usync"))]
mod std_mutex;
#[cfg(not(feature = "use-usync"))]
pub use std_mutex::{Flower, FlowerHandle};
