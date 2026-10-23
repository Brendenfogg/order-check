//! Order Check: verifies a spoken restaurant order against the POS ticket.

pub mod audio;
pub mod extract;
pub mod matcher;
pub mod menu;
pub mod normalize;
pub mod pipeline;
pub mod standards;
pub mod stt;
pub mod ticket;
pub mod verify;
