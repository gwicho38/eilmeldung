# Chyron Mode Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Bloomberg-style scrolling news chyron mode to dispatch with dashboard summary panel, scrolling ticker, and in-app mode switching.

**Architecture:** New `AppMode` enum (Reader/Chyron) gates rendering in the existing `Widget for &mut App` impl. Chyron state lives in a `ChyronState` struct on `App`. The existing message dispatch chain remains unchanged — chyron components simply aren't rendered when in Reader mode, and reader components aren't rendered in Chyron mode. A new `ChyronConfig` section is added to `Config` with `#[serde(default)]`.

**Tech Stack:** Rust, ratatui 0.30, news-flash 3.0, tokio, strum, serde, throbber-widgets-tui, webbrowser

**Spec:** `docs/specs/2026-03-13-chyron-mode-design.md`

---

## File Structure

### Modified Files

| File | Responsibility |
|------|---------------|
| `src/cli.rs` | Add `--chyron` flag as top-level `CliArgs` field |
| `src/main.rs` | Thread `AppMode` into `App::new()` based on `--chyron` flag |
| `src/ui/mod.rs` | Add `AppMode` enum, `mode` + `chyron_state` fields to `App`, tick/process chyron commands |
| `src/ui/view.rs` | Branch rendering on `self.mode` (Reader vs Chyron) |
| `src/messages/command/mod.rs` | Add 7 chyron command variants to `Command` enum |
| `src/input/mod.rs` | `InputCommandGenerator` gains `AppMode` awareness for mapping table selection |
| `src/config/mod.rs` | Add `chyron: ChyronConfig` field to `Config` struct |
| `src/config/input_config.rs` | Add `generate_default_chyron_commands()` function |
| `src/prelude.rs` | Re-export `AppMode` |

### New Files

| File | Responsibility |
|------|---------------|
| `src/ui/chyron/mod.rs` | `ChyronState` struct, `render_chyron()` method on `App`, chyron command handling |
| `src/ui/chyron/status_bar.rs` | `render_chyron_status_bar()` — 1-line status display |
| `src/ui/chyron/category_grid.rs` | `render_chyron_category_grid()` — responsive grid of category summary cells |
| `src/ui/chyron/ticker.rs` | `TickerState`, `TickerItem` structs, scroll/pause mechanics, rendering |
| `src/ui/chyron/ticker_queue.rs` | Round-robin queue fill logic, news-flash ArticleFilter queries |

---

## Chunk 1: Foundation — Types, Config, CLI

### Task 1: Add Chyron Command Variants

**Files:**
- Modify: `src/messages/command/mod.rs:204` (Command enum)

- [ ] **Step 1: Add the 7 chyron command variants to the `Command` enum**

Add these variants inside the `Command` enum at `src/messages/command/mod.rs`, after the existing variants (before the closing `}`). Follow the existing strum attribute pattern:

```rust
// Chyron mode
#[strum(
    serialize = "chyrontoggle",
    message = "chyrontoggle",
    detailed_message = "toggle between reader and chyron mode"
)]
ChyronToggle,

#[strum(
    serialize = "chyronpause",
    message = "chyronpause",
    detailed_message = "toggle pause/play in chyron mode"
)]
ChyronPause,

#[strum(
    serialize = "chyronspeedup",
    message = "chyronspeedup",
    detailed_message = "increase chyron scroll speed"
)]
ChyronSpeedUp,

#[strum(
    serialize = "chyronspeeddown",
    message = "chyronspeeddown",
    detailed_message = "decrease chyron scroll speed"
)]
ChyronSpeedDown,

#[strum(
    serialize = "chyronopencurrent",
    message = "chyronopencurrent",
    detailed_message = "open highlighted headline in browser (chyron paused)"
)]
ChyronOpenCurrent,

#[strum(
    serialize = "chyronprevheadline",
    message = "chyronprevheadline",
    detailed_message = "step to previous headline (chyron paused)"
)]
ChyronPrevHeadline,

#[strum(
    serialize = "chyronnextheadline",
    message = "chyronnextheadline",
    detailed_message = "step to next headline (chyron paused)"
)]
ChyronNextHeadline,
```

- [ ] **Step 2: Add `Display` impl arms for the new variants**

The `Display` impl for `Command` at line 732 is **exhaustive** (no `_ =>` catch-all). Add these arms inside the `match self.clone() { ... }` block, before the closing `}`:

```rust
ChyronToggle => write!(f, "toggle chyron mode"),
ChyronPause => write!(f, "toggle chyron pause"),
ChyronSpeedUp => write!(f, "increase chyron speed"),
ChyronSpeedDown => write!(f, "decrease chyron speed"),
ChyronOpenCurrent => write!(f, "open current headline"),
ChyronPrevHeadline => write!(f, "previous headline"),
ChyronNextHeadline => write!(f, "next headline"),
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`
Expected: compiles successfully

- [ ] **Step 4: Commit**

```bash
git add src/messages/command/mod.rs
git commit -m "feat(chyron): add chyron command variants to Command enum"
```

---

### Task 2: Add ChyronConfig to Config

**Files:**
- Modify: `src/config/mod.rs:113` (Config struct)
- Modify: `src/config/mod.rs:241` (Config Default impl)
- Modify: `src/config/input_config.rs` (add chyron default mappings)

- [ ] **Step 1: Add `ChyronInputConfig` to `src/config/input_config.rs`**

Add this struct **after** the `InputConfig` struct and its `Default` impl (follow the same module pattern — `InputConfig` and its defaults live here, so `ChyronInputConfig` should too):

```rust
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case", default)]
pub struct ChyronInputConfig {
    pub mappings: IndexMap<KeySequence, CommandSequence>,
}

impl Default for ChyronInputConfig {
    fn default() -> Self {
        Self {
            mappings: generate_default_chyron_commands(),
        }
    }
}
```

Then add `ChyronInputConfig` to the config prelude re-exports in `src/config/mod.rs` (where `InputConfig` is re-exported).

- [ ] **Step 1b: Add `ChyronConfig` struct to `src/config/mod.rs`**

Add `use std::collections::HashMap;` at the top of the file. Then add this struct before the `Config` struct (around line 110):

```rust
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct ChyronConfig {
    pub default_speed: u8,
    pub mark_as_read: bool,
    pub category_colors: HashMap<String, String>,
    pub input_config: ChyronInputConfig,
}

impl Default for ChyronConfig {
    fn default() -> Self {
        Self {
            default_speed: 5,
            mark_as_read: true,
            category_colors: HashMap::new(),
            input_config: ChyronInputConfig::default(),
        }
    }
}
```

- [ ] **Step 2: Add `chyron` field to `Config` struct**

In the `Config` struct at line ~113, add:

```rust
pub chyron: ChyronConfig,
```

And in the `Default` impl at line ~241, add:

```rust
chyron: ChyronConfig::default(),
```

- [ ] **Step 3: Add `generate_default_chyron_commands()` to `src/config/input_config.rs`**

Add this function after `generate_default_input_commands()`:

```rust
pub fn generate_default_chyron_commands() -> IndexMap<KeySequence, CommandSequence> {
    cmd_mappings! [
        "p"         => "chyronpause",
        "+"         => "chyronspeedup",
        "="         => "chyronspeedup",
        "-"         => "chyronspeeddown",
        "enter"     => "chyronopencurrent",
        "left"      => "chyronprevheadline",
        "right"     => "chyronnextheadline",
        "s"         => "sync",
        "C"         => "chyrontoggle",
        "q"         => "quit",
        "C-c"       => "quit",
    ]
}
```

- [ ] **Step 4: Add re-export in config prelude**

Check `src/config/mod.rs` prelude section and ensure `ChyronConfig` and `ChyronInputConfig` are accessible. They likely need to be `pub` and the module exports them since they're in `mod.rs` itself.

- [ ] **Step 5: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`
Expected: compiles successfully

- [ ] **Step 6: Commit**

```bash
git add src/config/mod.rs src/config/input_config.rs
git commit -m "feat(chyron): add ChyronConfig with default keybindings"
```

---

### Task 3: Add AppMode Enum and Wire Into App

**Files:**
- Modify: `src/ui/mod.rs:36` (add AppMode enum, add fields to App struct, update App::new signature)
- Modify: `src/prelude.rs` (re-export AppMode)

- [ ] **Step 1: Add `AppMode` enum to `src/ui/mod.rs`**

Add after the `AppState` enum (around line 42):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Reader,
    Chyron,
}
```

- [ ] **Step 2: Add `mode` field to `App` struct**

In the `App` struct at line ~171, add:

```rust
mode: AppMode,
```

- [ ] **Step 3: Update `App::new()` to accept `AppMode`**

Change the `App::new()` signature at line ~203 to:

```rust
pub fn new(
    config: Arc<Config>,
    news_flash_utils: Arc<NewsFlashUtils>,
    message_sender: UnboundedSender<Message>,
    mode: AppMode,
) -> Self {
```

And in the struct literal initialization, add:

```rust
mode,
```

- [ ] **Step 4: Update prelude re-export**

In `src/ui/mod.rs` line ~20, change:
```rust
pub use super::{App, AppState};
```
to:
```rust
pub use super::{App, AppMode, AppState};
```

- [ ] **Step 5: Update `src/main.rs` to pass `AppMode::Reader`**

At line 134, change:
```rust
let app = App::new(config.clone(), news_flash_utils.clone(), message_sender);
```
to:
```rust
let app = App::new(config.clone(), news_flash_utils.clone(), message_sender, AppMode::Reader);
```

This keeps existing behavior unchanged — `--chyron` flag will be wired in a later task.

- [ ] **Step 6: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`
Expected: compiles successfully

- [ ] **Step 7: Commit**

```bash
git add src/ui/mod.rs src/prelude.rs src/main.rs
git commit -m "feat(chyron): add AppMode enum and wire into App::new"
```

---

### Task 4: Add `--chyron` CLI Flag

**Files:**
- Modify: `src/cli.rs` (add `chyron` field to `CliArgs`)
- Modify: `src/main.rs` (check flag, pass correct `AppMode`)

- [ ] **Step 1: Add `--chyron` flag to `CliArgs`**

In `src/cli.rs`, add this field to the `CliArgs` struct (NOT inside `CliAction`). Add it after `quiet` (the last field before the closing `}`). Note: do NOT use `pub` — `CliArgs` uses `#[getset(get = "pub")]` which generates public getters for private fields:

```rust
/// Launch in chyron (scrolling ticker) mode
#[arg(long)]
chyron: bool,
```

- [ ] **Step 2: Wire the flag in `src/main.rs`**

At line ~134, change:
```rust
let app = App::new(config.clone(), news_flash_utils.clone(), message_sender, AppMode::Reader);
```
to:
```rust
let mode = if *cli_args.chyron() { AppMode::Chyron } else { AppMode::Reader };
let app = App::new(config.clone(), news_flash_utils.clone(), message_sender, mode);
```

Note: `cli_args.chyron()` returns `&bool` (getset getter), so dereference with `*`.

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`
Expected: compiles successfully

- [ ] **Step 4: Verify `--help` shows the flag**

Run: `cd /Users/home/repos/dispatch && cargo run -- --help 2>&1 | grep chyron`
Expected: `--chyron  Launch in chyron (scrolling ticker) mode`

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat(chyron): add --chyron CLI flag"
```

---

## Chunk 2: Chyron State and Ticker Core

### Task 5: Create Ticker Data Types

**Files:**
- Create: `src/ui/chyron/ticker.rs`
- Create: `src/ui/chyron/mod.rs` (initial skeleton)

- [ ] **Step 1: Create `src/ui/chyron/mod.rs` skeleton**

```rust
pub mod category_grid;
pub mod status_bar;
pub mod ticker;
pub mod ticker_queue;

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use news_flash::models::Url;
use ratatui::style::Color;

use crate::prelude::*;

use self::ticker::TickerState;

/// All mutable state for chyron mode, stored as a field on `App`.
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
```

- [ ] **Step 2: Create `src/ui/chyron/ticker.rs`**

```rust
use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use news_flash::models::Url;
use ratatui::style::Color;

/// A single headline in the ticker queue.
#[derive(Debug, Clone)]
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
            self.queue.get(self.highlight_index).map(|item| item.url.as_str())
        } else {
            None
        }
    }
}
```

- [ ] **Step 3: Create stub files for remaining modules**

Create `src/ui/chyron/status_bar.rs`:
```rust
// Chyron status bar rendering — implemented in Task 8
```

Create `src/ui/chyron/category_grid.rs`:
```rust
// Chyron category grid rendering — implemented in Task 9
```

Create `src/ui/chyron/ticker_queue.rs`:
```rust
// Round-robin ticker queue fill — implemented in Task 7
```

- [ ] **Step 4: Register the chyron module in `src/ui/mod.rs`**

Add at the top of `src/ui/mod.rs` (after the existing `mod` declarations):

```rust
pub mod chyron;
```

- [ ] **Step 5: Add `chyron_state` field to `App` struct**

In `src/ui/mod.rs`, add to the `App` struct:

```rust
chyron_state: chyron::ChyronState,
```

And in `App::new()`, add to the struct literal:

```rust
chyron_state: chyron::ChyronState::new(config.chyron.default_speed),
```

- [ ] **Step 6: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`
Expected: compiles successfully

- [ ] **Step 7: Commit**

```bash
git add src/ui/chyron/ src/ui/mod.rs
git commit -m "feat(chyron): add ChyronState and TickerState types"
```

---

### Task 6: InputCommandGenerator Mode Awareness

**Files:**
- Modify: `src/input/mod.rs:49` (InputCommandGenerator struct and process_key_event)
- Modify: `src/ui/mod.rs` (pass mode reference to InputCommandGenerator)

- [ ] **Step 1: Add `mode` field to `InputCommandGenerator`**

In `src/input/mod.rs`, add a field to the struct at line 49:

```rust
pub struct InputCommandGenerator {
    config: Arc<Config>,
    message_sender: UnboundedSender<Message>,
    key_sequence: KeySequence,
    last_input_instant: Instant,
    mode: AppMode,
}
```

- [ ] **Step 2: Update `InputCommandGenerator::new()` to accept and store `AppMode`**

```rust
pub fn new(config: Arc<Config>, message_sender: UnboundedSender<Message>, mode: AppMode) -> Self {
    Self {
        config,
        message_sender,
        key_sequence: KeySequence::default(),
        last_input_instant: Instant::now(),
        mode,
    }
}
```

- [ ] **Step 3: Add `set_mode()` method**

```rust
pub fn set_mode(&mut self, mode: AppMode) {
    self.mode = mode;
}
```

- [ ] **Step 4: Update `process_key_event` to use mode-aware mapping table**

In `process_key_event` at line ~157, the key lookup currently uses `self.config.input_config`. Change the mapping selection to be mode-aware.

Replace the `command` lookup at line ~162:
```rust
let command = key.as_ref().and_then(|key| {
    self.config
        .input_config
        .match_single_key_to_single_command(key)
});
```

With:
```rust
let mappings = match self.mode {
    AppMode::Reader => &self.config.input_config,
    AppMode::Chyron => &self.chyron_input_config(),
};
let command = key.as_ref().and_then(|key| {
    mappings.match_single_key_to_single_command(key)
});
```

And add a helper method:
```rust
fn chyron_input_config(&self) -> InputConfig {
    InputConfig {
        scroll_amount: self.config.input_config.scroll_amount,
        timeout_millis: self.config.input_config.timeout_millis,
        mappings: self.config.chyron.input_config.mappings.clone(),
        remove_unnecessary_mappings: false,
    }
}
```

Also update the `prefix_matches` lookup at line ~192 to use the same mode-aware mappings:
```rust
let active_mappings = match self.mode {
    AppMode::Reader => &self.config.input_config.mappings,
    AppMode::Chyron => &self.config.chyron.input_config.mappings,
};
let mut prefix_matches = active_mappings
    .iter()
    .filter(|(other_key_sequence, _)| self.key_sequence.is_prefix_of(other_key_sequence))
    .collect::<Vec<_>>();
```

And the direct match lookup at line ~206:
```rust
if let Some(command_sequence) = active_mappings.get(&self.key_sequence)
```

- [ ] **Step 5: Update `show_help_input` to use mode-aware mappings**

At line ~253, update to use mode-aware mappings instead of hardcoded `self.config.input_config.mappings`.

- [ ] **Step 6: Update `App::new()` in `src/ui/mod.rs` to pass mode**

Change:
```rust
input_command_generator: InputCommandGenerator::new(
    config_arc.clone(),
    message_sender.clone(),
),
```
to:
```rust
input_command_generator: InputCommandGenerator::new(
    config_arc.clone(),
    message_sender.clone(),
    mode,
),
```

- [ ] **Step 7: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`
Expected: compiles successfully

- [ ] **Step 8: Commit**

```bash
git add src/input/mod.rs src/ui/mod.rs
git commit -m "feat(chyron): make InputCommandGenerator mode-aware for keybinding selection"
```

---

### Task 7: Ticker Queue Fill Logic

**Files:**
- Implement: `src/ui/chyron/ticker_queue.rs`

- [ ] **Step 1: Implement round-robin queue fill**

Replace the stub in `src/ui/chyron/ticker_queue.rs`:

**IMPORTANT API notes (verified from codebase):**
- `news_flash.get_categories()` is **synchronous** (no `.await`), returns `Result<(Vec<Category>, Vec<CategoryMapping>)>`
- `news_flash.get_articles(filter)` is **synchronous** (no `.await`), takes `ArticleFilter` **by value** (not reference)
- `ArticleFilter` field for categories is **`categories`** (plural), set via `vec![cat_id].into()`
- `ArticleFilter` field for unread is **`unread`**, not `read`
- There is NO `get_unread_count_for_category()` method — use `unread_count_feed_map(true)?` to get per-feed counts, then aggregate by category using `CategoryMapping`
- See `src/ui/feeds_list/model.rs:206-211` and `src/ui/feeds_list/feed_list_item.rs:176-179` for patterns

```rust
use std::collections::{HashMap, VecDeque};

use log::{debug, trace};
use news_flash::models::{ArticleFilter, CategoryID, CategoryMapping, FeedID, Read as NfRead};
use ratatui::style::Color;

use crate::prelude::*;
use super::ticker::TickerItem;

/// Category metadata for round-robin cycling.
pub struct CategoryInfo {
    pub name: String,
    pub id: CategoryID,
    pub color: Color,
    pub unread_count: i64,
    pub latest_headline: Option<String>,
}

/// Default color palette for categories without explicit color mapping.
const DEFAULT_COLORS: &[Color] = &[
    Color::Green,
    Color::Blue,
    Color::Red,
    Color::Cyan,
    Color::Yellow,
    Color::Magenta,
    Color::LightGreen,
    Color::LightBlue,
    Color::LightRed,
    Color::LightCyan,
];

/// Resolve a color name string to a ratatui Color.
fn resolve_color(name: &str) -> Option<Color> {
    match name.to_lowercase().as_str() {
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "blue" => Some(Color::Blue),
        "cyan" => Some(Color::Cyan),
        "yellow" => Some(Color::Yellow),
        "magenta" => Some(Color::Magenta),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        "lightred" | "light_red" => Some(Color::LightRed),
        "lightgreen" | "light_green" => Some(Color::LightGreen),
        "lightblue" | "light_blue" => Some(Color::LightBlue),
        "lightcyan" | "light_cyan" => Some(Color::LightCyan),
        "lightyellow" | "light_yellow" => Some(Color::LightYellow),
        "lightmagenta" | "light_magenta" => Some(Color::LightMagenta),
        _ => None,
    }
}

/// Build the list of categories with their unread counts and assigned colors.
///
/// Pattern follows `src/ui/feeds_list/model.rs:92,206-211`:
/// 1. `get_categories()` returns `(Vec<Category>, Vec<CategoryMapping>)` (sync)
/// 2. `unread_count_feed_map(true)?` returns `HashMap<FeedID, i64>` (sync)
/// 3. Aggregate feed-level counts into category-level counts via CategoryMapping
pub async fn build_category_list(
    news_flash_utils: &NewsFlashUtils,
    config: &Config,
) -> Vec<CategoryInfo> {
    let news_flash = news_flash_utils.news_flash_lock.read().await;

    // Step 1: get categories (sync call)
    let (categories, category_mappings) = match news_flash.get_categories() {
        Ok(result) => result,
        Err(e) => {
            debug!("Failed to get categories: {}", e);
            return Vec::new();
        }
    };

    // Step 2: get per-feed unread counts (sync call)
    let feed_unread_map: HashMap<FeedID, i64> = match news_flash.unread_count_feed_map(true) {
        Ok(map) => map,
        Err(e) => {
            debug!("Failed to get unread counts: {}", e);
            HashMap::new()
        }
    };

    // Step 3: build category-to-feeds mapping from CategoryMapping
    let mut category_feed_map: HashMap<CategoryID, Vec<FeedID>> = HashMap::new();
    for mapping in &category_mappings {
        category_feed_map
            .entry(mapping.category_id.clone())
            .or_default()
            .push(mapping.feed_id.clone());
    }

    // Step 4: aggregate unread counts per category
    let mut result = Vec::new();
    for (idx, category) in categories.iter().enumerate() {
        let unread: i64 = category_feed_map
            .get(&category.category_id)
            .map(|feeds| {
                feeds.iter()
                    .map(|fid| feed_unread_map.get(fid).copied().unwrap_or(0))
                    .sum()
            })
            .unwrap_or(0);

        let color = config
            .chyron
            .category_colors
            .get(&category.label)
            .and_then(|c| resolve_color(c))
            .unwrap_or(DEFAULT_COLORS[idx % DEFAULT_COLORS.len()]);

        // Get latest headline for this category
        let latest_headline = {
            let filter = ArticleFilter {
                categories: vec![category.category_id.clone()].into(),
                ..Default::default()
            };
            news_flash.get_articles(filter)
                .ok()
                .and_then(|articles| articles.first().and_then(|a| a.title.clone()))
        };

        result.push(CategoryInfo {
            name: category.label.clone(),
            id: category.category_id.clone(),
            color,
            unread_count: unread,
            latest_headline,
        });
    }

    // Sort by unread count descending
    result.sort_by(|a, b| b.unread_count.cmp(&a.unread_count));
    result
}

/// Fetch the next batch of unread headlines from the given category.
///
/// Uses `ArticleFilter { categories: vec![cat_id].into(), unread: Some(Read::Unread), .. }`
/// Pattern from `src/ui/feeds_list/feed_list_item.rs:176-179`.
pub async fn fetch_category_headlines(
    news_flash_utils: &NewsFlashUtils,
    category: &CategoryInfo,
    limit: usize,
) -> Vec<TickerItem> {
    let news_flash = news_flash_utils.news_flash_lock.read().await;

    let filter = ArticleFilter {
        categories: vec![category.id.clone()].into(),
        unread: Some(NfRead::Unread),
        ..Default::default()
    };

    // get_articles takes filter by value, is sync (no .await)
    let articles = match news_flash.get_articles(filter) {
        Ok(articles) => articles,
        Err(e) => {
            debug!("Failed to get articles for category {}: {}", category.name, e);
            return Vec::new();
        }
    };

    articles
        .into_iter()
        .take(limit)
        .map(|article| TickerItem {
            category: category.name.clone(),
            color: category.color,
            feed_name: String::new(),
            title: article.title.clone().unwrap_or_default(),
            url: article.url.as_ref().map(|u| u.to_string()).unwrap_or_default(),
            article_id: Some(article.article_id.clone()),
            published: article.date,
        })
        .collect()
}

/// Refill the ticker queue using round-robin category cycling.
/// Called when queue depth drops below `min_depth`.
pub async fn refill_queue(
    queue: &mut VecDeque<TickerItem>,
    categories: &[CategoryInfo],
    current_category_index: &mut usize,
    news_flash_utils: &NewsFlashUtils,
    min_depth: usize,
    batch_size: usize,
) {
    if queue.len() >= min_depth || categories.is_empty() {
        return;
    }

    let mut attempts = 0;
    let max_attempts = categories.len();

    while queue.len() < min_depth && attempts < max_attempts {
        let cat = &categories[*current_category_index % categories.len()];
        *current_category_index = (*current_category_index + 1) % categories.len();
        attempts += 1;

        if cat.unread_count == 0 {
            continue;
        }

        let items = fetch_category_headlines(news_flash_utils, cat, batch_size).await;
        trace!(
            "Fetched {} headlines from category {}",
            items.len(),
            cat.name
        );
        for item in items {
            queue.push_back(item);
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -10`
Expected: compiles (may have warnings about unused imports which is fine — rendering code will use them)

- [ ] **Step 3: Commit**

```bash
git add src/ui/chyron/ticker_queue.rs
git commit -m "feat(chyron): implement round-robin ticker queue fill logic"
```

---

## Chunk 3: Rendering

### Task 8: Status Bar Rendering

**Files:**
- Implement: `src/ui/chyron/status_bar.rs`

- [ ] **Step 1: Implement the status bar renderer**

Replace the stub in `src/ui/chyron/status_bar.rs`:

```rust
use chrono::{DateTime, Utc};
use ratatui::prelude::*;
use throbber_widgets_tui::{Throbber, ThrobberState, BRAILLE_EIGHT_DOUBLE};

use crate::prelude::*;

/// Render the single-line chyron status bar.
///
/// Content: `░ DISPATCH CHYRON ░  {feed_count} feeds │ {unread_count} unread │ {●/○} │ synced {timestamp}`
pub fn render_chyron_status_bar(
    area: Rect,
    buf: &mut Buffer,
    config: &Config,
    feed_count: usize,
    unread_count: i64,
    is_syncing: bool,
    is_offline: bool,
    last_sync_time: Option<DateTime<Utc>>,
    throbber_state: &ThrobberState,
) {
    // Fill background
    Block::default()
        .style(config.theme.statusbar())
        .render(area, buf);

    let connection_indicator = if is_offline { "○" } else { "●" };

    let sync_text = if is_syncing {
        let throbber = Throbber::default()
            .throbber_style(config.theme.statusbar())
            .style(config.theme.statusbar())
            .throbber_set(BRAILLE_EIGHT_DOUBLE)
            .use_type(throbber_widgets_tui::WhichUse::Spin);
        " syncing...".to_string()
    } else {
        match last_sync_time {
            Some(time) => format!(" synced {}", time.format("%H:%M")),
            None => " not synced".to_string(),
        }
    };

    let status_text = format!(
        " ░ DISPATCH CHYRON ░  {} feeds │ {} unread │ {} │{}",
        feed_count, unread_count, connection_indicator, sync_text
    );

    let line = Line::from(Span::styled(status_text, config.theme.statusbar()));
    line.render(area, buf);
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`

- [ ] **Step 3: Commit**

```bash
git add src/ui/chyron/status_bar.rs
git commit -m "feat(chyron): implement status bar rendering"
```

---

### Task 9: Category Grid Rendering

**Files:**
- Implement: `src/ui/chyron/category_grid.rs`

- [ ] **Step 1: Implement the category grid renderer**

Replace the stub in `src/ui/chyron/category_grid.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::prelude::*;
use super::ticker_queue::CategoryInfo;

/// Render a responsive grid of category summary cells.
///
/// Column count adapts to terminal width: 4 columns at 120+, 3 at 80+, 2 at narrow.
/// Categories are pre-sorted by unread count descending.
pub fn render_chyron_category_grid(
    area: Rect,
    buf: &mut Buffer,
    categories: &[CategoryInfo],
    config: &Config,
) {
    if categories.is_empty() {
        let msg = Paragraph::new("No categories found. Press s to sync.")
            .style(config.theme.paragraph())
            .alignment(Alignment::Center);
        msg.render(area, buf);
        return;
    }

    let col_count = if area.width >= 120 {
        4
    } else if area.width >= 80 {
        3
    } else {
        2
    };

    let row_count = (categories.len() + col_count - 1) / col_count;
    let cell_width = area.width / col_count as u16;
    let cell_height = if row_count > 0 {
        (area.height / row_count as u16).max(3)
    } else {
        3
    };

    for (idx, cat) in categories.iter().enumerate() {
        let col = idx % col_count;
        let row = idx / col_count;

        let x = area.x + (col as u16) * cell_width;
        let y = area.y + (row as u16) * cell_height;

        // Skip if we'd render off-screen
        if y + cell_height > area.y + area.height {
            break;
        }

        let cell_area = Rect::new(
            x,
            y,
            cell_width.min(area.x + area.width - x),
            cell_height.min(area.y + area.height - y),
        );

        render_category_cell(cell_area, buf, cat, config);
    }
}

fn render_category_cell(
    area: Rect,
    buf: &mut Buffer,
    category: &CategoryInfo,
    config: &Config,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(category.color))
        .title(Span::styled(
            truncate_str(&category.name, area.width.saturating_sub(2) as usize),
            Style::default().fg(category.color).bold(),
        ));

    let inner = block.inner(area);
    block.render(area, buf);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    // Line 1: unread count
    let count_text = format!("{} unread", category.unread_count);
    let count_line = Line::from(Span::styled(
        count_text,
        Style::default().fg(Color::White),
    ));
    if inner.height >= 1 {
        count_line.render(Rect::new(inner.x, inner.y, inner.width, 1), buf);
    }

    // Line 2: most recent headline (truncated)
    if inner.height >= 2 {
        if let Some(headline) = &category.latest_headline {
            let truncated = truncate_str(headline, inner.width as usize);
            let headline_line = Line::from(Span::styled(
                truncated,
                Style::default().fg(Color::DarkGray),
            ));
            headline_line.render(Rect::new(inner.x, inner.y + 1, inner.width, 1), buf);
        }
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 1 {
        format!("{}…", &s[..max_len - 1])
    } else {
        String::new()
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`

- [ ] **Step 3: Commit**

```bash
git add src/ui/chyron/category_grid.rs
git commit -m "feat(chyron): implement category grid rendering"
```

---

### Task 10: Ticker Line Rendering

**Files:**
- Modify: `src/ui/chyron/ticker.rs` (add rendering function)

- [ ] **Step 1: Add ticker rendering function to `src/ui/chyron/ticker.rs`**

Add this function at the bottom of the file:

```rust
use ratatui::prelude::*;
use crate::prelude::*;

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
    let paragraph = ratatui::widgets::Paragraph::new(full_line)
        .scroll((0, state.scroll_offset as u16));

    paragraph.render(area, buf);
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`

- [ ] **Step 3: Commit**

```bash
git add src/ui/chyron/ticker.rs
git commit -m "feat(chyron): implement ticker line rendering with scroll and highlight"
```

---

### Task 11: Chyron Layout and `render_chyron` Method

**Files:**
- Modify: `src/ui/chyron/mod.rs` (add `render_chyron` as method on `App`)
- Modify: `src/ui/view.rs` (branch on `AppMode`)

- [ ] **Step 1: Implement `render_chyron` in `src/ui/chyron/mod.rs`**

Add to `src/ui/chyron/mod.rs` — an impl block for `App`:

```rust
use ratatui::prelude::*;

impl App {
    /// Render the complete chyron layout: status bar, category grid, ticker, help bar.
    pub fn render_chyron(&mut self, area: Rect, buf: &mut Buffer) {
        // 4-zone vertical layout
        let [status_area, grid_area, ticker_area, help_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // status bar
                Constraint::Min(0),    // category grid (fills remaining)
                Constraint::Length(2), // ticker
                Constraint::Length(1), // help bar
            ])
            .areas(area);

        // Status bar (uses cached category data from ChyronState)
        status_bar::render_chyron_status_bar(
            status_area,
            buf,
            &self.config,
            self.chyron_state.feed_count,
            self.chyron_state.total_unread,
            self.news_flash_utils.is_async_operation_running(),
            self.is_offline,
            self.chyron_state.last_sync_time,
            &self.async_operation_throbber,
        );

        // Category grid (uses cached categories from ChyronState)
        category_grid::render_chyron_category_grid(
            grid_area,
            buf,
            &self.chyron_state.categories,
            &self.config,
        );

        // Ticker
        ticker::render_ticker(
            ticker_area,
            buf,
            &self.chyron_state.ticker,
            &self.config,
        );

        // Help bar
        let help_line = if self.chyron_state.ticker.paused {
            Line::from(Span::styled(
                " ▌▌ Paused │ ←/→ prev/next │ ↵ open │ p resume │ q quit",
                self.config.theme.statusbar(),
            ))
        } else {
            Line::from(Span::styled(
                format!(
                    " ▶ Playing (speed {}) │ +/- speed │ p pause │ s sync │ C reader │ q quit",
                    self.chyron_state.ticker.speed
                ),
                self.config.theme.statusbar(),
            ))
        };

        Block::default()
            .style(self.config.theme.statusbar())
            .render(help_area, buf);
        help_line.render(help_area, buf);
    }
}
```

- [ ] **Step 2: Add `AppMode` branch to `src/ui/view.rs`**

In `src/ui/view.rs`, wrap the existing rendering code in a `match self.mode`:

```rust
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.mode {
            AppMode::Chyron => {
                self.render_chyron(area, buf);
                return;
            }
            AppMode::Reader => {
                // all existing rendering code below (unchanged)
            }
        }

        // ... existing Reader rendering code stays here inside the Reader arm ...
    }
}
```

Restructure: move ALL existing code inside the `render` method into the `AppMode::Reader` match arm. The `AppMode::Chyron` arm calls `self.render_chyron(area, buf)` and returns early.

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -10`

- [ ] **Step 4: Commit**

```bash
git add src/ui/chyron/mod.rs src/ui/view.rs
git commit -m "feat(chyron): implement chyron layout rendering with mode branching"
```

---

## Chunk 4: Command Handling and Data Flow

### Task 12: Handle Chyron Commands in App

**Files:**
- Modify: `src/ui/mod.rs` (add chyron command handling in `MessageReceiver for App`)

- [ ] **Step 1: Add chyron command handling to `App::process_command`**

In `src/ui/mod.rs`, add new match arms in the `MessageReceiver for App` impl (inside `process_command`, before the `_ =>` catch-all):

```rust
Message::Command(Command::ChyronToggle) => {
    match self.mode {
        AppMode::Reader => {
            self.mode = AppMode::Chyron;
            self.chyron_state = chyron::ChyronState::new(self.config.chyron.default_speed);
            self.input_command_generator.set_mode(AppMode::Chyron);
        }
        AppMode::Chyron => {
            self.mode = AppMode::Reader;
            self.input_command_generator.set_mode(AppMode::Reader);
        }
    }
}

Message::Command(Command::ChyronPause) if self.mode == AppMode::Chyron => {
    self.chyron_state.ticker.toggle_pause();
}

Message::Command(Command::ChyronSpeedUp) if self.mode == AppMode::Chyron => {
    self.chyron_state.ticker.speed_up();
}

Message::Command(Command::ChyronSpeedDown) if self.mode == AppMode::Chyron => {
    self.chyron_state.ticker.speed_down();
}

Message::Command(Command::ChyronOpenCurrent) if self.mode == AppMode::Chyron => {
    if let Some(url) = self.chyron_state.ticker.highlighted_url() {
        let url_owned = url.to_string();
        if let Err(e) = webbrowser::open(&url_owned) {
            tooltip(
                &self.message_sender,
                &format!("Failed to open browser: {}", e),
                TooltipFlavor::Error,
            )?;
        }
    }
}

Message::Command(Command::ChyronPrevHeadline) if self.mode == AppMode::Chyron => {
    self.chyron_state.ticker.prev_headline();
}

Message::Command(Command::ChyronNextHeadline) if self.mode == AppMode::Chyron => {
    self.chyron_state.ticker.next_headline();
}
```

- [ ] **Step 2: Update `tick()` to advance chyron scroll**

In `src/ui/mod.rs`, update the `tick()` method at line ~321:

```rust
fn tick(&mut self) -> bool {
    // Always update throbber when async operation is running (both modes)
    if self.news_flash_utils.is_async_operation_running() {
        self.async_operation_throbber.calc_next();
    }

    if self.mode == AppMode::Chyron {
        self.chyron_state.ticker.advance();
        return true; // always redraw in chyron mode (ticker is animating)
    }

    // Reader mode: only redraw when throbber is active
    self.news_flash_utils.is_async_operation_running()
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -10`

- [ ] **Step 4: Commit**

```bash
git add src/ui/mod.rs
git commit -m "feat(chyron): handle chyron commands in App message processing"
```

---

### Task 13: Wire Data Flow — Categories + Queue Refresh

**Files:**
- Modify: `src/ui/chyron/mod.rs` (add cached categories, refresh on sync)
- Modify: `src/ui/mod.rs` (trigger queue refill and category refresh)

- [ ] **Step 1: Add cached categories to `ChyronState`**

Update `ChyronState` in `src/ui/chyron/mod.rs`:

```rust
pub struct ChyronState {
    pub ticker: TickerState,
    pub last_sync_time: Option<DateTime<Utc>>,
    pub categories: Vec<ticker_queue::CategoryInfo>,
    pub total_unread: i64,
    pub feed_count: usize,
}

impl ChyronState {
    pub fn new(default_speed: u8) -> Self {
        Self {
            ticker: TickerState::new(default_speed),
            last_sync_time: None,
            categories: Vec::new(),
            total_unread: 0,
            feed_count: 0,
        }
    }
}
```

- [ ] **Step 2: Add async category + queue refresh method to `App`**

In `src/ui/mod.rs`, add a method on `App`:

```rust
async fn refresh_chyron_data(&mut self) {
    if self.mode != AppMode::Chyron {
        return;
    }

    // Refresh category list
    self.chyron_state.categories = chyron::ticker_queue::build_category_list(
        &self.news_flash_utils,
        &self.config,
    ).await;

    self.chyron_state.total_unread = self.chyron_state.categories
        .iter()
        .map(|c| c.unread_count)
        .sum();

    // Get actual feed count (not category count)
    let news_flash = self.news_flash_utils.news_flash_lock.read().await;
    self.chyron_state.feed_count = news_flash.get_feeds()
        .map(|feeds| feeds.len())
        .unwrap_or(0);
    drop(news_flash);
    self.chyron_state.last_sync_time = Some(Utc::now());

    // Refill ticker queue
    chyron::ticker_queue::refill_queue(
        &mut self.chyron_state.ticker.queue,
        &self.chyron_state.categories,
        &mut self.chyron_state.ticker.current_category_index,
        &self.news_flash_utils,
        5,
        10,
    ).await;
}
```

Add `use chrono::Utc;` at the top of the file if not already present.

- [ ] **Step 3: Call `refresh_chyron_data` on `AsyncSyncFinished` and `ApplicationStarted`**

In the `MessageReceiver for App` impl, add to the `AsyncSyncFinished` handler:

```rust
Message::Event(Event::AsyncSyncFinished(..)) => {
    // existing code...
    info!(...);
    self.batch_processor.show_popup();
    self.message_sender.send(Message::Batch(self.config.after_sync_commands.to_vec()))?;

    // Refresh chyron data after sync
    self.refresh_chyron_data().await;
}
```

And in the `ApplicationStarted` handler (add a new match arm if one doesn't exist):

```rust
Message::Event(Event::ApplicationStarted) => {
    self.refresh_chyron_data().await;
    needs_redraw = false; // the initial sync will trigger a redraw
}
```

- [ ] **Step 4: Refill queue on tick when depth is low**

In `tick()`, add queue refill check. Since `tick()` is synchronous but `refill_queue` is async, we need to send a message instead. Add a new simple approach — check queue depth in `process_command` on `Tick`:

In the `Message::Event(Tick)` handler in `App::process_command`, add:

```rust
Message::Event(Tick) => {
    needs_redraw = self.tick();

    // In chyron mode, check if queue needs refilling
    if self.mode == AppMode::Chyron && self.chyron_state.ticker.queue.len() < 5 {
        chyron::ticker_queue::refill_queue(
            &mut self.chyron_state.ticker.queue,
            &self.chyron_state.categories,
            &mut self.chyron_state.ticker.current_category_index,
            &self.news_flash_utils,
            5,
            10,
        ).await;
    }
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -10`

- [ ] **Step 6: Commit**

```bash
git add src/ui/chyron/mod.rs src/ui/mod.rs
git commit -m "feat(chyron): wire data flow for categories and ticker queue refresh"
```

---

## Chunk 5: Polish and Edge Cases

### Task 14: Mark-as-Read on Scroll-Off

**Files:**
- Modify: `src/ui/chyron/ticker.rs` (return popped item info for mark-as-read)
- Modify: `src/ui/mod.rs` (mark article as read when item scrolls off)

- [ ] **Step 1: Track headline width for scroll-off detection**

In `src/ui/chyron/ticker.rs`, add a method to `TickerState`:

```rust
/// Calculate the display width of the first item in the queue (including separator).
/// When scroll_offset exceeds this, the item has scrolled off screen.
pub fn first_item_width(&self) -> usize {
    if let Some(item) = self.queue.front() {
        // "[CATEGORY] Title" + separator " ███ "
        let tag = format!("[{}] ", item.category);
        tag.len() + item.title.len() + 5 // 5 = " ███ " separator
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
```

- [ ] **Step 2: Handle mark-as-read in tick processing**

In `src/ui/mod.rs`, update the Tick handler to check for scrolled-off items:

```rust
// After the queue refill check, add:
if self.mode == AppMode::Chyron {
    while let Some(_popped) = self.chyron_state.ticker.check_and_pop_scrolled_off() {
        // Mark as read if configured
        if self.config.chyron.mark_as_read {
            // Note: we'd need the ArticleID to mark as read.
            // For now, this is a placeholder — the TickerItem stores a URL string
            // but not the ArticleID. We'll skip mark-as-read in v1 and add it
            // when TickerItem gets an article_id field.
        }
    }
}
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`

- [ ] **Step 4: Commit**

```bash
git add src/ui/chyron/ticker.rs src/ui/mod.rs
git commit -m "feat(chyron): add scroll-off detection and history tracking"
```

---

### Task 15: Integration Test — Full Build and Manual Verification

**Files:** None (verification only)

- [ ] **Step 1: Full cargo build**

Run: `cd /Users/home/repos/dispatch && cargo build 2>&1 | tail -10`
Expected: builds successfully with no errors

- [ ] **Step 2: Run existing tests**

Run: `cd /Users/home/repos/dispatch && cargo test 2>&1 | tail -10`
Expected: all existing tests pass

- [ ] **Step 3: Verify `--chyron` flag is available**

Run: `cd /Users/home/repos/dispatch && cargo run -- --help 2>&1 | grep -A1 chyron`
Expected: shows `--chyron` flag in help output

- [ ] **Step 4: Verify `C` keybinding works for toggle**

Manual test: run `cargo run`, press `C` to toggle to chyron mode, verify the layout changes. Press `C` again to return to reader mode. Press `q` to quit.

- [ ] **Step 5: Verify `--chyron` starts in chyron mode**

Manual test: run `cargo run -- --chyron`, verify it starts directly in chyron mode with the 4-zone layout.

- [ ] **Step 6: Commit any fixes**

If any issues were found, fix and commit:
```bash
git add -A
git commit -m "fix(chyron): address integration issues from manual testing"
```

---

### Task 16: Config Validation

**Files:**
- Modify: `src/config/mod.rs` (add chyron config validation)

- [ ] **Step 1: Add chyron validation in `Config::validate()`**

In `src/config/mod.rs`, add to the `validate()` method at line ~194:

```rust
if self.chyron.default_speed == 0 || self.chyron.default_speed > 10 {
    return Err(color_eyre::eyre::eyre!(
        "chyron.default_speed must be between 1 and 10"
    ));
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/home/repos/dispatch && cargo check 2>&1 | tail -5`

- [ ] **Step 3: Commit**

```bash
git add src/config/mod.rs
git commit -m "feat(chyron): add chyron config validation"
```
