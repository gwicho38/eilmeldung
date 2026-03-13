use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use ratatui::style::Color;

/// A single headline in the ticker queue.
#[derive(Debug, Clone)]
#[allow(dead_code)] // fields used in upcoming chyron rendering tasks
pub struct TickerItem {
    pub category: String,
    pub color: Color,
    pub feed_name: String,
    pub title: String,
    pub url: String,
    pub article_id: Option<news_flash::models::ArticleID>,
    pub published: Option<DateTime<Utc>>,
}

/// Mutable state for the scrolling ticker.
#[allow(dead_code)] // fields used in upcoming chyron rendering tasks
pub struct TickerState {
    pub queue: VecDeque<TickerItem>,
    pub history: VecDeque<TickerItem>,
    pub scroll_offset: usize,
    pub speed: u8,
    pub paused: bool,
    pub highlight_index: usize,
    pub current_category_index: usize,
}

impl TickerState {
    pub fn new(default_speed: u8) -> Self {
        Self {
            queue: VecDeque::new(),
            history: VecDeque::with_capacity(20),
            scroll_offset: 0,
            speed: default_speed.clamp(1, 10),
            paused: false,
            highlight_index: 0,
            current_category_index: 0,
        }
    }

    /// Advance the scroll offset by `speed` characters. Called on each tick when not paused.
    #[allow(dead_code)] // called in upcoming chyron tick handler
    pub fn advance(&mut self) {
        if !self.paused {
            self.scroll_offset += self.speed as usize;
        }
    }

    #[allow(dead_code)] // called in upcoming chyron command handler
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if self.paused {
            self.highlight_index = 0;
        }
    }

    #[allow(dead_code)] // called in upcoming chyron command handler
    pub fn speed_up(&mut self) {
        self.speed = (self.speed + 1).min(10);
    }

    #[allow(dead_code)] // called in upcoming chyron command handler
    pub fn speed_down(&mut self) {
        self.speed = self.speed.saturating_sub(1).max(1);
    }

    /// Step to the next headline when paused.
    #[allow(dead_code)] // called in upcoming chyron command handler
    pub fn next_headline(&mut self) {
        if self.paused && !self.queue.is_empty() {
            self.highlight_index = (self.highlight_index + 1).min(self.queue.len() - 1);
        }
    }

    /// Step to the previous headline when paused.
    /// If at the front of the queue, pulls from history.
    #[allow(dead_code)] // called in upcoming chyron command handler
    pub fn prev_headline(&mut self) {
        if !self.paused {
            return;
        }
        if self.highlight_index > 0 {
            self.highlight_index -= 1;
        } else if let Some(item) = self.history.pop_front() {
            self.queue.push_front(item);
            // highlight_index stays at 0 (now pointing to the recovered item)
        }
    }

    /// Pop the frontmost item (scrolled off-screen) into history.
    #[allow(dead_code)] // called in upcoming chyron tick handler
    pub fn pop_front_to_history(&mut self) {
        if let Some(item) = self.queue.pop_front() {
            if self.history.len() >= 20 {
                self.history.pop_back();
            }
            self.history.push_front(item);
        }
    }

    /// Get the URL of the currently highlighted item (for opening in browser).
    #[allow(dead_code)] // called in upcoming chyron command handler
    pub fn highlighted_url(&self) -> Option<&str> {
        if self.paused {
            self.queue
                .get(self.highlight_index)
                .map(|item| item.url.as_str())
        } else {
            None
        }
    }
}
