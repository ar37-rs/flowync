#[cfg(feature = "parking-lot")]
mod parking_lot_mutex;
#[cfg(feature = "parking-lot")]
pub use parking_lot_mutex::{Flower, FlowerHandle};

#[cfg(not(feature = "parking-lot"))]
mod std_mutex;
#[cfg(not(feature = "parking-lot"))]
pub use std_mutex::{Flower, FlowerHandle};
