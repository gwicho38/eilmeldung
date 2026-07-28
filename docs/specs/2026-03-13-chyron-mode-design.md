# Chyron Mode — Design Spec

**Date:** 2026-03-13
**Status:** Approved
**Scope:** Add a Bloomberg-style scrolling news chyron mode to dispatch

---

## Overview

A new ambient "chyron" mode that transforms dispatch from an interactive RSS reader into a passive, scrolling news ticker with a dashboard summary panel. Designed for "leave it running in a tmux pane" awareness across 145+ categorized RSS feeds spanning finance, security, politics, tech, and more.

## Layout

```
┌─────────────────────────────────────────────────────┐
│  ░ DISPATCH CHYRON  ░  145 feeds │ 312 unread │ ● │  ← status bar (1 line)
├─────────────────────────────────────────────────────┤
│ ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌─────────┐ │
│ │ FINANCE  │ │ INTL NEWS│ │ INTEL    │ │ TECH    │ │  ← category grid
│ │ 23 unread│ │ 87 unread│ │ 41 unread│ │ 12 unrd │ │    (fills middle)
│ │ Matt Lev…│ │ NYT: Ukr…│ │ Krebs: … │ │ Ars: …  │ │
│ ├──────────┤ ├──────────┤ ├──────────┤ ├─────────┤ │
│ │ PYTHON   │ │ POLITICS │ │ UAP      │ │ RADIO   │ │
│ │  4 unread│ │ 18 unread│ │  9 unread│ │  3 unrd │ │
│ │ Real Py… │ │ ProPub…  │ │ Debrief… │ │ Priyom… │ │
│ └──────────┘ └──────────┘ └──────────┘ └─────────┘ │
├─────────────────────────────────────────────────────┤
│ [FINANCE] Matt Levine: The SEC Has Thoughts About ← │  ← scrolling ticker (1-2 lines)
├─────────────────────────────────────────────────────┤
│  ▶ Playing  │ +/- speed │ p pause │ ↵ open │ q quit │  ← help bar (1 line)
└─────────────────────────────────────────────────────┘
```

### Layout Zones

| Zone | Height | Content |
|------|--------|---------|
| Status bar | 1 line | App name, feed count, unread count, connection indicator, last sync time |
| Category grid | Fills remaining | Responsive grid of category summary cells |
| Scrolling ticker | 1-2 lines | Single-lane, color-coded, round-robin headline scroller |
| Help bar | 1 line | Current state + available keybindings |

## New Types

### AppMode

```rust
// src/ui/mod.rs (or src/ui/app_mode.rs)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Reader,
    Chyron,
}
```

`AppMode` is added as a field on the `App` struct. It determines which top-level layout is rendered. The existing `AppState` enum (`FeedSelection`, `ArticleSelection`, `ArticleContent`, `ArticleContentDistractionFree`) is **frozen** when `AppMode::Chyron` is active — its value is preserved but not read or updated. When switching back to `AppMode::Reader`, the frozen `AppState` is resumed.

## Activation

### CLI Flag

```bash
dispatch --chyron
```

The `--chyron` flag is a **top-level field on `CliArgs`**, not part of the `CliAction` arg group. `CliAction` members (`--sync`, `--stats`, `--export-opml`, etc.) are mutually exclusive one-shot actions that exit after running. `--chyron` instead launches the full TUI event loop with `AppMode::Chyron` as the initial mode.

In `main.rs`, after `execute_cli_actions()` returns `false` (no one-shot action), the `--chyron` flag is checked and threaded into `App::new()`:

```rust
// src/cli.rs
#[derive(Parser)]
pub struct CliArgs {
    // ... existing fields ...

    /// Launch in chyron (scrolling ticker) mode
    #[arg(long)]
    pub chyron: bool,
}

// src/main.rs
let mode = if cli_args.chyron { AppMode::Chyron } else { AppMode::Reader };
let app = App::new(/* existing args */, mode);
```

### In-App Toggle

Press `C` to switch between reader mode and chyron mode. Both modes share the same `NewsFlash` instance and SQLite database. Reader state (selected feed, article, scroll position) is preserved when switching to chyron and restored when switching back.

## Components

### 1. ChyronStatusBar

Single-line status display.

**Content:** `░ DISPATCH CHYRON ░  {feed_count} feeds │ {unread_count} unread │ {●/○} │ synced {timestamp}`

**Updates on:**
- `Event::AsyncSyncFinished` — refreshes unread count and sync timestamp
- `Event::ConnectionAvailable` / `Event::ConnectionLost` — toggles connection indicator
- During sync: shows throbber spinner (reuses existing `throbber-widgets-tui` dependency)

### 2. ChyronCategoryGrid

Responsive grid of category summary cells filling the middle area.

**Each cell shows:**
- Category name (color-coded)
- Unread article count
- Most recent headline (truncated to fit cell width)

**Behavior:**
- Column count adapts to terminal width: 4 columns at 120+ chars, 3 at 80+, 2 at narrow
- Categories sorted by unread count descending (most active top-left)
- Re-queries news-flash on `Event::AsyncSyncFinished`

### 3. ChyronTicker

Single-lane scrolling headline ticker.

**State:**

```rust
struct TickerState {
    queue: VecDeque<TickerItem>,
    history: VecDeque<TickerItem>,  // bounded ring buffer (last 20 items) for backward nav
    scroll_offset: usize,          // integer character-cell offset
    speed: u8,                     // 1-10, default 5 (characters per tick)
    paused: bool,
    highlight_index: usize,        // which item is "current" when paused
    current_category_index: usize, // round-robin pointer
}

struct TickerItem {
    category: String,
    color: Color,
    feed_name: String,
    title: String,
    url: Url,                      // news_flash::models::Url for type safety
    published: DateTime<Utc>,
}
```

**Round-robin queue fill:**
- When queue depth drops below 5 items, fetches the next batch of unread headlines from the next category in rotation
- Uses `news_flash.get_articles(ArticleFilter)` with a category constraint to fetch unread articles sorted by `published` descending, limited to 10 per batch
- Skips categories with 0 unread articles
- Wraps around when all categories are exhausted; re-checks for newly synced articles

**Scroll mechanics:**
- On each `Event::Tick` (at app's configured FPS), advances `scroll_offset` by `speed` characters
- `scroll_offset` is `usize` — terminal rendering operates on integer character cells, no subpixel logic needed
- When a headline fully scrolls off the left edge, it is popped from the queue and pushed into `history` (capped at 20)
- If `chyron.mark_as_read` is true, the popped headline is marked as read in news-flash

**Pause navigation:**
- When paused, `highlight_index` tracks the currently selected headline
- Right arrow: `highlight_index += 1` (step forward in queue)
- Left arrow: if `highlight_index` would go below 0, pop from `history` back into the front of `queue` and display it
- Current headline rendered with reversed color style

**Rendered format:**
```
[FINANCE] Matt Levine: The SEC Has Thoughts ███ [SECURITY] Krebs: New Zero-Day in...
```

Category tags are color-coded. Separator between headlines is a configurable gap (3 block chars default).

### 4. Help Bar

Single-line contextual help.

**Playing state:** `▶ Playing │ +/- speed │ p pause │ s sync │ C reader │ q quit`

**Paused state:** `▌▌ Paused │ ←/→ prev/next │ ↵ open │ p resume │ q quit`

## Keybindings (Chyron Mode)

| Key | Command | Description |
|-----|---------|-------------|
| `p` | `ChyronPause` | Toggle pause/play |
| `+` / `=` | `ChyronSpeedUp` | Increase scroll speed (max 10) |
| `-` | `ChyronSpeedDown` | Decrease scroll speed (min 1) |
| `Enter` | `ChyronOpenCurrent` | Open highlighted headline in browser (paused only) |
| `←` | `ChyronPrevHeadline` | Step to previous headline (paused only, pulls from history) |
| `→` | `ChyronNextHeadline` | Step to next headline (paused only) |
| `s` | `FeedListSync` | Trigger manual sync (reuses existing command) |
| `C` | `ChyronToggle` | Switch to reader mode |
| `q` | `Quit` | Quit application |

### Keybinding Architecture

The existing `InputCommandGenerator` uses a flat `IndexMap<KeySequence, CommandSequence>` with no modal context. Chyron mode keybindings are implemented as follows:

- `InputCommandGenerator` gains a reference to the current `AppMode`
- Before looking up a key in the mapping table, it checks `AppMode`:
  - `AppMode::Reader` → uses existing `[input_config.mappings]`
  - `AppMode::Chyron` → uses a new `[chyron.input_config.mappings]` table
- Default chyron mappings are hardcoded (p/+/-/Enter/arrows/s/C/q) and can be overridden in config

```toml
# Optional overrides in config.toml
[chyron.input_config.mappings]
"p"     = ["chyronpause"]
"+"     = ["chyronspeedup"]
"-"     = ["chyronspeeddown"]
"enter" = ["chyronopencurrent"]
"left"  = ["chyronprevheadline"]
"right" = ["chyronnextheadline"]
"s"     = ["sync"]
"C"     = ["chyrontoggle"]
"q"     = ["quit"]
```

## Data Flow

```
Event::Tick
  → ChyronTicker checks queue depth
  → If low: news_flash.get_articles(ArticleFilter { category, unread, limit: 10 })
  → Advance scroll_offset by speed characters
  → Render visible portion of queue as single-line Paragraph widget

Event::AsyncSyncFinished
  → ChyronCategoryGrid refreshes all category counts + latest headlines
  → ChyronStatusBar updates unread total + sync timestamp
  → ChyronTicker queue picks up new articles on next refill cycle
```

## Rendering Architecture

### ChyronView Integration

The existing `Widget for &mut App` impl in `src/ui/view.rs` directly renders the 3-panel layout. There is no separate `ReaderView` struct. `ChyronView` follows the same pattern — it is **not** a standalone struct but a code path within the existing `Widget for &mut App` impl:

```rust
// src/ui/view.rs
impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.mode {
            AppMode::Reader => {
                // existing 3-panel render code (unchanged)
            }
            AppMode::Chyron => {
                // delegate to chyron rendering
                self.render_chyron(area, buf);
            }
        }
    }
}
```

`render_chyron` is implemented in `src/ui/chyron/mod.rs` as a method on `App` (via a trait or direct impl). It accesses `App` fields: `self.news_flash_utils`, `self.config`, `self.chyron_state` (new field), `self.connectivity_state`.

### Background Component Processing

When `AppMode::Chyron` is active, existing components (`feed_list`, `articles_list`, `article_content`) **continue to receive and process messages** through the dispatch chain. This is by design:
- `FeedListSync` command works because `FeedList` handles it regardless of mode
- Sync results flow through the existing pipeline naturally
- Performance impact is negligible (message processing is cheap; only rendering is skipped)

No gating is needed — the components simply aren't rendered, so their state updates are invisible but correct.

## Integration with Existing Codebase

### Changes to Existing Files

| File | Change |
|------|--------|
| `src/cli.rs` | Add `--chyron` as top-level `CliArgs` field (NOT inside `CliAction` group) |
| `src/main.rs` | Check `cli_args.chyron` after `execute_cli_actions`, pass `AppMode` to `App::new()` |
| `src/ui/mod.rs` | Add `AppMode` enum, add `mode: AppMode` and `chyron_state: ChyronState` fields to `App` |
| `src/ui/view.rs` | Match on `self.mode` in `Widget for &mut App` to branch rendering |
| `src/messages/command/mod.rs` | Add `ChyronToggle`, `ChyronPause`, `ChyronSpeedUp`, `ChyronSpeedDown`, `ChyronOpenCurrent`, `ChyronPrevHeadline`, `ChyronNextHeadline` variants |
| `src/input/mod.rs` | `InputCommandGenerator` gains `AppMode` awareness; selects reader or chyron mapping table |
| `src/config/mod.rs` | Add `chyron: ChyronConfig` field to `Config` struct (required since `Config` uses `deny_unknown_fields`) |

### New Files

```
src/ui/chyron/
├── mod.rs              # render_chyron impl, ChyronState, owns sub-components
├── status_bar.rs       # ChyronStatusBar rendering
├── category_grid.rs    # ChyronCategoryGrid rendering
├── ticker.rs           # ChyronTicker + TickerState + TickerItem
└── ticker_queue.rs     # Round-robin queue logic, ArticleFilter queries
```

### Files NOT Modified

- `src/ui/feeds_list/` — untouched
- `src/ui/articles_list/` — untouched
- `src/ui/article_content/` — untouched
- `src/newsflash_utils.rs` — used read-only via existing API
- News-flash library — no modifications

## Configuration

### ChyronConfig Struct

```rust
// Added to src/config/mod.rs
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ChyronConfig {
    pub default_speed: u8,                          // 1-10, default 5
    pub mark_as_read: bool,                         // default true
    pub category_colors: HashMap<String, String>,   // category tag → color name
    pub input_config: Option<ChyronInputConfig>,    // optional keybinding overrides
}
```

Added as `chyron: ChyronConfig` field on the existing `Config` struct. Uses `#[serde(default)]` so the entire `[chyron]` section is optional.

### Auto-Sync

Chyron mode reuses the existing `sync_every_minutes` config field (already in `Config` at `src/config/mod.rs`). No separate `chyron.auto_sync_minutes` field — this avoids ambiguity about which timer takes precedence. If the user wants more frequent syncing in chyron mode, they set `sync_every_minutes` globally.

### config.toml Example

```toml
sync_every_minutes = 10   # existing field, used by both modes

[chyron]
default_speed = 5
mark_as_read = true

[chyron.category_colors]
"Financial-Markets-&-Trading" = "green"
"Intelligence-&-OSINT" = "red"
"International-News" = "blue"
"Tech-News" = "cyan"
"Politics-&-Government-Transparency" = "yellow"
"AI-&-Machine-Learning" = "magenta"
# Unspecified categories cycle through theme.color_palette accents
```

## Opening Headlines in Browser

`ChyronOpenCurrent` (Enter key when paused) uses the existing `webbrowser` crate (already in `Cargo.toml`) via `webbrowser::open(url)`. This is consistent with the existing `ActionOpenInBrowser` command path used in reader mode.

## Edge Cases

### Empty States
- **No unread articles:** Ticker shows `No new headlines. Press s to sync.` as static centered text
- **Category has 0 unread:** Round-robin skips silently
- **All categories exhausted:** Wraps around, re-checks for newly synced articles

### Sync During Chyron Mode
- Auto-sync runs on the global `sync_every_minutes` interval
- Status bar shows throbber during sync
- New articles available to ticker queue on next refill — no scroll interruption

### Pause Behavior
- Pausing freezes `scroll_offset` advancement
- Left/right arrow keys step through headlines one at a time
- Left arrow pulls from `history` ring buffer (last 20 popped items) when stepping past the front of the queue
- Current headline gets reversed color highlight
- `Enter` opens highlighted headline via `webbrowser::open(url)`

### Terminal Resize
- Category grid recalculates column count (ratatui `Layout` handles this)
- Ticker line reflows — `scroll_offset` is relative to headline string, not terminal width

### Mode Switching
- Chyron → Reader: reader state (`AppState`, selected feed, article, scroll) is preserved and restored
- Reader → Chyron: builds fresh `TickerState`, queries current unread counts from SQLite
