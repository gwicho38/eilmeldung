use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use ratatui::prelude::*;
use ratatui::style::Color;
use ratatui::widgets::Paragraph;

use crate::prelude::*;

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
    pub fn advance(&mut self) {
        if !self.paused {
            self.scroll_offset += self.speed as usize;
        }
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if self.paused {
            self.highlight_index = 0;
        }
    }

    pub fn speed_up(&mut self) {
        self.speed = (self.speed + 1).min(10);
    }

    pub fn speed_down(&mut self) {
        self.speed = self.speed.saturating_sub(1).max(1);
    }

    /// Step to the next headline when paused.
    pub fn next_headline(&mut self) {
        if self.paused && !self.queue.is_empty() {
            self.highlight_index = (self.highlight_index + 1).min(self.queue.len() - 1);
        }
    }

    /// Step to the previous headline when paused.
    /// If at the front of the queue, pulls from history.
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

/// Render the scrolling ticker line.
///
/// Format: `[CATEGORY] Title ███ [CATEGORY] Title ███ ...`
/// The separator is 3 block characters.
pub fn render_ticker(
    area: Rect,
    buf: &mut Buffer,
    state: &TickerState,
    config: &Config,
) {
    if state.queue.is_empty() {
        let msg = Line::from(Span::styled(
            "No new headlines. Press s to sync.",
            config.theme.paragraph(),
        ));
        msg.render(area, buf);
        return;
    }

    let separator = "███";
    let mut spans: Vec<Span<'_>> = Vec::new();

    for (idx, item) in state.queue.iter().enumerate() {
        if !spans.is_empty() {
            spans.push(Span::styled(
                format!(" {} ", separator),
                Style::default().fg(Color::DarkGray),
            ));
        }

        let tag_style = if state.paused && idx == state.highlight_index {
            Style::default().fg(Color::Black).bg(item.color).bold()
        } else {
            Style::default().fg(item.color).bold()
        };

        let title_style = if state.paused && idx == state.highlight_index {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::White)
        };

        spans.push(Span::styled(format!("[{}] ", item.category), tag_style));
        spans.push(Span::styled(item.title.clone(), title_style));
    }

    // Build the full text line and handle horizontal scrolling
    let full_line = Line::from(spans);

    // For scrolling: we render from scroll_offset onward
    // Ratatui's Paragraph with scroll handles this
    let paragraph = Paragraph::new(full_line)
        .scroll((0, state.scroll_offset as u16));

    paragraph.render(area, buf);
}
