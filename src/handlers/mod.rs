/// Handler modules for Discord events and interactions
mod voice;
mod channel;
mod interaction;
mod birthday;

// Re-export main handler functions
pub use voice::handle_voice_state_update;
pub use interaction::{handle_interaction, handle_modal_submit};
