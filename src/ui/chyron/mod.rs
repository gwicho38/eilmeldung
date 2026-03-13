pub mod category_grid;
pub mod status_bar;
pub mod ticker;
pub mod ticker_queue;

use chrono::{DateTime, Utc};

use self::ticker::TickerState;

/// All mutable state for chyron mode, stored as a field on `App`.
#[allow(dead_code)] // fields used in upcoming chyron rendering tasks
pub struct ChyronState {
    pub ticker: TickerState,
    pub last_sync_time: Option<DateTime<Utc>>,
}

impl ChyronState {
    pub fn new(default_speed: u8) -> Self {
        Self {
            ticker: TickerState::new(default_speed),
            last_sync_time: None,
        }
    }
}
