use std::collections::VecDeque;

use crate::prelude::*;
use chrono::{DateTime, Utc};
use ratatui::prelude::*;
use ratatui::style::Color;

/// A single headline in the ticker queue.
#[derive(Debug, Clone)]
pub struct TickerItem {
    pub category: String,
    pub color: Color,
    #[allow(dead_code)] // planned for ticker display
    pub feed_name: String,
    pub title: String,
    pub summary: Option<String>,
    pub url: String,
    #[allow(dead_code)] // planned for mark-as-read in v2
    pub article_id: Option<news_flash::models::ArticleID>,
    #[allow(dead_code)] // planned for time display in v2
    pub published: Option<DateTime<Utc>>,
}

/// Speed presets mapping speed level (1-10) to chars-per-tick as a fraction.
/// At 60 FPS chyron tick rate: level 1 ≈ 10 chars/sec (advance every ~6 frames = 100ms).
const SPEED_TABLE: [f32; 10] = [0.17, 0.25, 0.33, 0.5, 0.67, 1.0, 1.5, 2.0, 3.0, 5.0];

/// Mutable state for the scrolling ticker.
pub struct TickerState {
    pub queue: VecDeque<TickerItem>,
    pub history: VecDeque<TickerItem>,
    pub scroll_offset: usize,
    pub speed: u8,
    pub paused: bool,
    pub highlight_index: usize,
    pub current_category_index: usize,
    /// Fractional accumulator for sub-character scrolling.
    scroll_accumulator: f32,
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
            scroll_accumulator: 0.0,
        }
    }

    /// Advance the scroll offset using fractional accumulation. Called on each tick when not paused.
    pub fn advance(&mut self) {
        if !self.paused && !self.queue.is_empty() {
            let rate = SPEED_TABLE[(self.speed as usize - 1).min(SPEED_TABLE.len() - 1)];
            self.scroll_accumulator += rate;
            let whole = self.scroll_accumulator as usize;
            if whole > 0 {
                self.scroll_offset += whole;
                self.scroll_accumulator -= whole as f32;
            }
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

    /// Calculate the display width of the first item in the queue (including separator).
    /// When scroll_offset exceeds this, the item has scrolled off screen.
    pub fn first_item_width(&self) -> usize {
        if let Some(item) = self.queue.front() {
            // "[CATEGORY] Title | summary" + separator " ███ "
            let tag = format!("[{}] ", item.category);
            let summary_len = item.summary.as_ref().map(|s| 3 + s.len()).unwrap_or(0); // 3 = " | "
            tag.len() + item.title.len() + summary_len + 5 // 5 = " ███ " separator
        } else {
            0
        }
    }

    /// Check if the first item has scrolled off screen and needs to be popped.
    /// Returns the popped item if one was removed.
    pub fn check_and_pop_scrolled_off(&mut self) -> Option<TickerItem> {
        let width = self.first_item_width();
        if width > 0 && self.scroll_offset >= width {
            self.scroll_offset -= width;
            let item = self.queue.pop_front();
            if let Some(ref popped) = item {
                if self.history.len() >= 20 {
                    self.history.pop_back();
                }
                self.history.push_front(popped.clone());
            }
            item
        } else {
            None
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

/// Render the scrolling ticker via direct buffer cell writes.
///
/// Bypasses Paragraph widget overhead for minimal per-frame allocation.
/// Format: `[CATEGORY] Title | summary ███ [CATEGORY] Title | summary ███ ...`
pub fn render_ticker(area: Rect, buf: &mut Buffer, state: &TickerState, config: &Config) {
    if state.queue.is_empty() {
        let msg = Line::from(Span::styled(
            "No new headlines. Press s to sync.",
            config.theme.paragraph(),
        ));
        msg.render(area, buf);
        return;
    }

    let width = area.width as usize;
    if width == 0 || area.height == 0 {
        return;
    }

    let styled_chars = build_styled_chars(state);
    let total_len = styled_chars.len();
    let y = area.y;

    for c in 0..width {
        let content_idx = state.scroll_offset + c;
        if content_idx < total_len {
            let (ch, style) = styled_chars[content_idx];
            if let Some(cell) = buf.cell_mut(Position::new(area.x + c as u16, y)) {
                cell.set_char(ch).set_style(style);
            }
        }
    }
}

/// Flatten the ticker queue into a vector of (char, Style) pairs for direct buffer rendering.
fn build_styled_chars(state: &TickerState) -> Vec<(char, Style)> {
    let separator = " ███ ";
    let mut chars = Vec::new();

    for (idx, item) in state.queue.iter().enumerate() {
        if idx > 0 {
            let sep_style = Style::default().fg(Color::DarkGray);
            for ch in separator.chars() {
                chars.push((ch, sep_style));
            }
        }

        let is_highlighted = state.paused && idx == state.highlight_index;

        let tag_style = if is_highlighted {
            Style::default().fg(Color::Black).bg(item.color).bold()
        } else {
            Style::default().fg(item.color).bold()
        };

        let title_style = if is_highlighted {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::White)
        };

        for ch in format!("[{}] ", item.category).chars() {
            chars.push((ch, tag_style));
        }
        for ch in item.title.chars() {
            chars.push((ch, title_style));
        }

        if let Some(summary) = &item.summary {
            let pipe_style = Style::default().fg(Color::DarkGray);
            for ch in " | ".chars() {
                chars.push((ch, pipe_style));
            }
            let summary_style = if is_highlighted {
                Style::default().fg(Color::Black).bg(Color::Gray)
            } else {
                Style::default().fg(Color::Gray)
            };
            for ch in summary.chars() {
                chars.push((ch, summary_style));
            }
        }
    }

    chars
}
