//! Order Check: verifies a spoken restaurant order against the POS ticket.

pub mod audio;
pub mod extract;
pub mod matcher;
pub mod menu;
pub mod mic;
pub mod normalize;
pub mod pipeline;
pub mod records;
pub mod server;
pub mod session;
pub mod standards;
pub mod stt;
pub mod ticket;
pub mod verify;
