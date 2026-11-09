use crate::error::{AppError, Result};
use crate::history::{Conversation, LoaderMessage, hidden};
use crate::search;
use crate::tui::ui;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::prelude::*;
use std::cell::Cell;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// Max gap between two clicks on the same item to count as a double-click
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// Result of running the TUI
pub enum Action {
    Resume {
        session_id: String,
        project_path: PathBuf,
        /// The session's last permission mode, restored on resume
        permission_mode: Option<String>,
    },
    Quit,
}

/// Loading state for the TUI
#[derive(Clone, Debug)]
pub enum LoadingState {
    Loading { loaded: usize },
    Ready,
}

/// App state
pub struct App {
    conversations: Vec<Conversation>,
    /// Indices into `conversations` that match the query, in display order
    filtered: Vec<usize>,
    selected: Option<usize>,
    query: String,
    query_words: Vec<String>,
    /// Cursor position in `query`, in chars
    cursor_pos: usize,
    loading_state: LoadingState,
    /// Path armed by the first Ctrl+D; a second Ctrl+D on the same item hides it
    pending_hide: Option<PathBuf>,
    /// List area from the last render, used to map mouse clicks to items
    list_area: Cell<Rect>,
    /// Time and list index of the last left click, for double-click detection
    last_click: Option<(Instant, usize)>,
}

impl App {
    pub fn new_loading() -> Self {
        Self {
            conversations: Vec::new(),
            filtered: Vec::new(),
            selected: None,
            query: String::new(),
            query_words: Vec::new(),
            cursor_pos: 0,
            loading_state: LoadingState::Loading { loaded: 0 },
            pending_hide: None,
            list_area: Cell::new(Rect::default()),
            last_click: None,
        }
    }

    pub fn append_conversations(&mut self, new_convs: Vec<Conversation>) {
        let start_idx = self.conversations.len();
        self.conversations.extend(new_convs);
        self.filtered.extend(start_idx..self.conversations.len());
        if self.selected.is_none() && !self.filtered.is_empty() {
            self.selected = Some(0);
        }
        self.loading_state = LoadingState::Loading {
            loaded: self.conversations.len(),
        };
    }

    pub fn finish_loading(&mut self) {
        self.conversations
            .sort_by_key(|c| std::cmp::Reverse(c.timestamp));
        self.loading_state = LoadingState::Ready;
        self.update_filter();
    }

    pub fn loading_state(&self) -> &LoadingState {
        &self.loading_state
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.loading_state, LoadingState::Loading { .. })
    }

    /// Re-split the query; once loaded, also re-filter the list. While loading
    /// the list stays unfiltered and only highlighting follows the query.
    fn on_query_changed(&mut self) {
        self.query_words = search::query_words(&self.query);
        if !self.is_loading() {
            self.update_filter();
        }
    }

    fn update_filter(&mut self) {
        self.filtered = search::search(&self.conversations, &self.query_words);
        self.selected = (!self.filtered.is_empty()).then_some(0);
    }

    /// Move the selection by `delta`, clamped to the list
    fn select_by(&mut self, delta: isize) {
        if let Some(selected) = self.selected {
            let last = self.filtered.len().saturating_sub(1);
            self.selected = Some(selected.saturating_add_signed(delta).min(last));
        }
    }

    fn selected_conversation(&self) -> Option<&Conversation> {
        let &idx = self.filtered.get(self.selected?)?;
        Some(&self.conversations[idx])
    }

    fn resume_selected(&self) -> Option<Action> {
        let conv = self.selected_conversation()?;
        Some(Action::Resume {
            session_id: hidden::session_id(&conv.path)?.to_owned(),
            project_path: conv.project_path.clone(),
            permission_mode: conv.permission_mode.clone(),
        })
    }

    pub fn filtered(&self) -> &[usize] {
        &self.filtered
    }

    pub fn conversations(&self) -> &[Conversation] {
        &self.conversations
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn query_words(&self) -> &[String] {
        &self.query_words
    }

    pub fn set_list_area(&self, area: Rect) {
        self.list_area.set(area);
    }

    /// Map a screen position to an index into `filtered`
    fn item_at(&self, column: u16, row: u16) -> Option<usize> {
        let area = self.list_area.get();
        if !area.contains(Position::new(column, row)) {
            return None;
        }
        let items_per_page = area.height as usize / ui::LINES_PER_ITEM;
        let row_in_page = (row - area.y) as usize / ui::LINES_PER_ITEM;
        if row_in_page >= items_per_page {
            return None;
        }
        let idx = ui::page_offset(self.selected, items_per_page) + row_in_page;
        (idx < self.filtered.len()).then_some(idx)
    }

    /// Handle a mouse event: wheel moves the selection, click selects, double-click resumes
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Action> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.pending_hide = None;
                self.select_by(-1);
            }
            MouseEventKind::ScrollDown => {
                self.pending_hide = None;
                self.select_by(1);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.pending_hide = None;
                let idx = self.item_at(mouse.column, mouse.row)?;
                self.selected = Some(idx);

                let now = Instant::now();
                let is_double_click = matches!(
                    self.last_click,
                    Some((at, last_idx)) if last_idx == idx && now.duration_since(at) < DOUBLE_CLICK
                );
                if is_double_click && !self.is_loading() {
                    self.last_click = None;
                    return self.resume_selected();
                }
                self.last_click = Some((now, idx));
            }
            _ => {}
        }
        None
    }

    pub fn is_hide_pending(&self) -> bool {
        self.pending_hide.is_some()
    }

    /// Ctrl+D: the first press arms hiding the selected conversation, a second
    /// press on the same one hides it.
    fn press_hide(&mut self, armed: Option<PathBuf>) {
        let current = self.selected_conversation().map(|c| c.path.clone());
        if current.is_some() && armed == current {
            self.hide_selected();
        } else {
            self.pending_hide = current;
        }
    }

    /// Record the selected conversation as hidden, then drop it from the list.
    /// Claude Code's files are left untouched; the list is only updated if recording succeeds.
    fn hide_selected(&mut self) {
        let Some(selected) = self.selected else {
            return;
        };
        let conv_idx = self.filtered[selected];
        let Some(id) = hidden::session_id(&self.conversations[conv_idx].path) else {
            return;
        };
        if hidden::hide(id).is_err() {
            return;
        }

        self.conversations.remove(conv_idx);
        self.filtered.remove(selected);
        for idx in self.filtered.iter_mut() {
            if *idx > conv_idx {
                *idx -= 1;
            }
        }
        self.selected = self
            .filtered
            .len()
            .checked_sub(1)
            .map(|last| selected.min(last));
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Byte offset of the char at `char_pos` in the query (its length at the end)
    fn byte_at(&self, char_pos: usize) -> usize {
        self.query
            .char_indices()
            .nth(char_pos)
            .map_or(self.query.len(), |(i, _)| i)
    }

    /// Remove the query chars in `start..end` (char positions), leaving the
    /// cursor at `start`. Returns whether anything was removed.
    fn remove_chars(&mut self, start: usize, end: usize) -> bool {
        if start >= end {
            return false;
        }
        let range = self.byte_at(start)..self.byte_at(end);
        self.query.replace_range(range, "");
        self.cursor_pos = start;
        true
    }

    /// Char position where the word before the cursor starts (for Ctrl+W)
    fn word_start_before_cursor(&self) -> usize {
        let chars: Vec<char> = self.query.chars().take(self.cursor_pos).collect();
        let mut pos = chars.len();
        while pos > 0 && search::is_word_separator(chars[pos - 1]) {
            pos -= 1;
        }
        while pos > 0 && !search::is_word_separator(chars[pos - 1]) {
            pos -= 1;
        }
        pos
    }

    /// Handle a key event, returns Some(Action) if the app should exit.
    /// While loading, the query can be edited but nothing can be resumed or hidden.
    pub fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        // Any key other than a second Ctrl+D cancels a pending hide
        let armed = self.pending_hide.take();
        let ctrl = modifiers.contains(KeyModifiers::CONTROL);
        let ready = !self.is_loading();
        let cursor = self.cursor_pos;

        let query_changed = match code {
            KeyCode::Esc => return Some(Action::Quit),
            KeyCode::Char('c') if ctrl => return Some(Action::Quit),
            KeyCode::Enter if ready => return self.resume_selected(),

            KeyCode::Char('d') if ctrl => {
                if ready {
                    self.press_hide(armed);
                }
                false
            }
            KeyCode::Char('w') if ctrl => {
                self.remove_chars(self.word_start_before_cursor(), cursor)
            }
            KeyCode::Char(c) => {
                let byte_pos = self.byte_at(cursor);
                self.query.insert(byte_pos, c);
                self.cursor_pos += 1;
                true
            }
            KeyCode::Backspace if cursor > 0 => self.remove_chars(cursor - 1, cursor),
            KeyCode::Delete => {
                self.remove_chars(cursor, (cursor + 1).min(self.query.chars().count()))
            }

            KeyCode::Left => {
                self.cursor_pos = cursor.saturating_sub(1);
                false
            }
            KeyCode::Right => {
                self.cursor_pos = (cursor + 1).min(self.query.chars().count());
                false
            }
            KeyCode::Up => {
                self.select_by(-1);
                false
            }
            KeyCode::Down => {
                self.select_by(1);
                false
            }
            KeyCode::Home if ready => {
                self.select_by(isize::MIN);
                false
            }
            KeyCode::End if ready => {
                self.select_by(isize::MAX);
                false
            }
            KeyCode::PageUp if ready => {
                self.select_by(-10);
                false
            }
            KeyCode::PageDown if ready => {
                self.select_by(10);
                false
            }
            _ => false,
        };

        if query_changed {
            self.on_query_changed();
        }
        None
    }
}

/// Undo raw mode, mouse capture and the alternate screen
fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = crossterm::execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
}

/// RAII guard to ensure terminal is restored on exit
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let setup = crossterm::execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)
            .and_then(|()| Terminal::new(CrosstermBackend::new(io::stdout())));
        match setup {
            Ok(terminal) => Ok(Self { terminal }),
            Err(e) => {
                restore_terminal();
                Err(e.into())
            }
        }
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

/// Run the TUI while conversations stream in from `rx`
pub fn run_with_loader(rx: Receiver<LoaderMessage>) -> Result<Action> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal();
        original_hook(panic_info);
    }));

    let mut guard = TerminalGuard::new()?;
    let mut app = App::new_loading();
    let mut dirty = true;

    loop {
        // Process all pending loader messages (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(LoaderMessage::Batch(convs)) => {
                    app.append_conversations(convs);
                    dirty = true;
                }
                Ok(LoaderMessage::Fatal(err)) => return Err(err),
                Ok(LoaderMessage::Done) | Err(TryRecvError::Disconnected) => {
                    if app.is_loading() {
                        app.finish_loading();
                        dirty = true;
                        if app.conversations().is_empty() {
                            return Err(AppError::NoHistoryFound);
                        }
                    }
                    break;
                }
                Err(TryRecvError::Empty) => break,
            }
        }

        // Redraw only when something changed
        if dirty {
            guard.terminal.draw(|frame| ui::render(frame, &app))?;
            dirty = false;
        }

        if event::poll(Duration::from_millis(50))? {
            let event = event::read()?;
            dirty = !matches!(&event, Event::Mouse(m) if m.kind == MouseEventKind::Moved);
            let action = match event {
                // Only handle key press events (not release)
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key.code, key.modifiers)
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => None,
            };
            if let Some(action) = action {
                return Ok(action);
            }
        }
    }
}
