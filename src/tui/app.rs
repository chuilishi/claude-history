use crate::error::{AppError, Result};
use crate::history::{hidden, Conversation, LoaderMessage};
use crate::tui::search::{self, SearchableConversation};
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
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// Max gap between two clicks on the same item to count as a double-click
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// Result of running the TUI
pub enum Action {
    Resume(PathBuf),
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
    searchable: Vec<SearchableConversation>,
    filtered: Vec<usize>,
    selected: Option<usize>,
    query: String,
    query_words: Vec<String>,
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
            searchable: Vec::new(),
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
        let end_idx = self.conversations.len();

        self.filtered.extend(start_idx..end_idx);

        if self.selected.is_none() && !self.filtered.is_empty() {
            self.selected = Some(0);
        }

        self.loading_state = LoadingState::Loading {
            loaded: self.conversations.len(),
        };
    }

    pub fn finish_loading(&mut self) {
        self.conversations
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        for (idx, conv) in self.conversations.iter_mut().enumerate() {
            conv.index = idx;
        }

        self.searchable = search::precompute_search_text(&self.conversations);
        self.loading_state = LoadingState::Ready;

        if self.query.is_empty() {
            self.filtered = (0..self.conversations.len()).collect();
            self.selected = if self.filtered.is_empty() {
                None
            } else {
                Some(0)
            };
        } else {
            self.update_filter();
        }
    }

    pub fn into_conversations(self) -> Vec<Conversation> {
        self.conversations
    }

    pub fn loading_state(&self) -> &LoadingState {
        &self.loading_state
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.loading_state, LoadingState::Loading { .. })
    }

    fn refresh_query_words(&mut self) {
        let query_normalized = search::normalize_for_search(self.query.trim());
        self.query_words = query_normalized
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
    }

    fn update_filter(&mut self) {
        let now = chrono::Local::now();
        self.filtered = search::search(&self.conversations, &self.searchable, &self.query, now);
        self.selected = if self.filtered.is_empty() {
            None
        } else {
            Some(0)
        };
        self.refresh_query_words();
    }

    fn select_prev(&mut self) {
        if let Some(selected) = self.selected
            && selected > 0
        {
            self.selected = Some(selected - 1);
        }
    }

    fn select_next(&mut self) {
        if let Some(selected) = self.selected
            && selected + 1 < self.filtered.len()
        {
            self.selected = Some(selected + 1);
        }
    }

    fn select_first(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = Some(0);
        }
    }

    fn select_last(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = Some(self.filtered.len() - 1);
        }
    }

    fn select_page_up(&mut self) {
        if let Some(selected) = self.selected {
            self.selected = Some(selected.saturating_sub(10));
        }
    }

    fn select_page_down(&mut self) {
        if let Some(selected) = self.selected {
            let new_selected = (selected + 10).min(self.filtered.len().saturating_sub(1));
            self.selected = Some(new_selected);
        }
    }

    fn get_selected_path(&self) -> Option<PathBuf> {
        self.selected
            .and_then(|sel| self.filtered.get(sel))
            .map(|&idx| self.conversations[idx].path.clone())
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

    /// Map a screen position to an index into `filtered`, mirroring the paging in `ui::render_list`
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
        let offset = self
            .selected
            .map(|sel| (sel / items_per_page) * items_per_page)
            .unwrap_or(0);
        let idx = offset + row_in_page;
        (idx < self.filtered.len()).then_some(idx)
    }

    /// Handle a mouse event: wheel moves the selection, click selects, double-click resumes
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Action> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.pending_hide = None;
                self.select_prev();
                None
            }
            MouseEventKind::ScrollDown => {
                self.pending_hide = None;
                self.select_next();
                None
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
                    return self.get_selected_path().map(Action::Resume);
                }
                self.last_click = Some((now, idx));
                None
            }
            _ => None,
        }
    }

    pub fn is_hide_pending(&self) -> bool {
        self.pending_hide.is_some()
    }

    /// Record the selected conversation as hidden, then drop it from the list.
    /// Claude Code's files are left untouched; the list is only updated if recording succeeds.
    fn hide_selected(&mut self) {
        let Some(selected) = self.selected else {
            return;
        };
        let Some(&conv_idx) = self.filtered.get(selected) else {
            return;
        };
        let Some(id) = hidden::session_id(&self.conversations[conv_idx].path) else {
            return;
        };
        if hidden::hide(id).is_err() {
            return;
        }

        self.conversations.remove(conv_idx);
        self.searchable.remove(conv_idx);
        for (idx, conv) in self.conversations.iter_mut().enumerate() {
            conv.index = idx;
        }
        for (idx, s) in self.searchable.iter_mut().enumerate() {
            s.index = idx;
        }

        self.filtered.remove(selected);
        for idx in self.filtered.iter_mut() {
            if *idx > conv_idx {
                *idx -= 1;
            }
        }

        self.selected = if self.filtered.is_empty() {
            None
        } else {
            Some(selected.min(self.filtered.len() - 1))
        };
    }

    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    fn cursor_right(&mut self) {
        let len = self.query.chars().count();
        if self.cursor_pos < len {
            self.cursor_pos += 1;
        }
    }

    fn delete_word_backwards(&mut self) -> bool {
        let chars: Vec<char> = self.query.chars().collect();
        let cursor = self.cursor_pos.min(chars.len());
        if cursor == 0 {
            return false;
        }

        let mut new_pos = cursor;

        while new_pos > 0 && search::is_word_separator(chars[new_pos - 1]) {
            new_pos -= 1;
        }

        while new_pos > 0 && !search::is_word_separator(chars[new_pos - 1]) {
            new_pos -= 1;
        }

        if new_pos == cursor {
            return false;
        }

        let start_byte = self
            .query
            .char_indices()
            .nth(new_pos)
            .map(|(i, _)| i)
            .unwrap_or(0);

        let end_byte = self
            .query
            .char_indices()
            .nth(cursor)
            .map(|(i, _)| i)
            .unwrap_or(self.query.len());

        self.query.replace_range(start_byte..end_byte, "");
        self.cursor_pos = new_pos;
        true
    }

    /// Handle a key event, returns Some(Action) if the app should exit
    pub fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
    ) -> Option<Action> {
        if self.is_loading() {
            return self.handle_loading_key(code, modifiers);
        }

        // Any key other than a second Ctrl+D cancels a pending hide
        let armed = self.pending_hide.take();

        match code {
            KeyCode::Esc => Some(Action::Quit),
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),

            // Enter = resume selected conversation
            KeyCode::Enter => self.get_selected_path().map(Action::Resume),

            KeyCode::Left => {
                self.cursor_left();
                None
            }
            KeyCode::Right => {
                self.cursor_right();
                None
            }
            KeyCode::Up => {
                self.select_prev();
                None
            }
            KeyCode::Down => {
                self.select_next();
                None
            }
            KeyCode::Home => {
                self.select_first();
                None
            }
            KeyCode::End => {
                self.select_last();
                None
            }
            KeyCode::PageUp => {
                self.select_page_up();
                None
            }
            KeyCode::PageDown => {
                self.select_page_down();
                None
            }
            KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                if self.delete_word_backwards() {
                    self.update_filter();
                }
                None
            }
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                let current = self.get_selected_path();
                if current.is_some() && armed == current {
                    self.hide_selected();
                } else {
                    self.pending_hide = current;
                }
                None
            }
            KeyCode::Char(c) => {
                let byte_pos = self
                    .query
                    .char_indices()
                    .nth(self.cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(self.query.len());
                self.query.insert(byte_pos, c);
                self.cursor_pos += 1;
                self.update_filter();
                None
            }
            KeyCode::Backspace => {
                let mut changed = false;
                if self.cursor_pos > 0
                    && let Some((byte_pos, _)) = self.query.char_indices().nth(self.cursor_pos - 1)
                {
                    self.query.remove(byte_pos);
                    self.cursor_pos -= 1;
                    changed = true;
                }
                if changed {
                    self.update_filter();
                }
                None
            }
            KeyCode::Delete => {
                let mut changed = false;
                let len = self.query.chars().count();
                if self.cursor_pos < len
                    && let Some((byte_pos, _)) = self.query.char_indices().nth(self.cursor_pos)
                {
                    self.query.remove(byte_pos);
                    changed = true;
                }
                if changed {
                    self.update_filter();
                }
                None
            }
            _ => None,
        }
    }

    /// Handle keys during loading (limited set)
    fn handle_loading_key(&mut self, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        match code {
            KeyCode::Esc => Some(Action::Quit),
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
            KeyCode::Left => {
                self.cursor_left();
                None
            }
            KeyCode::Right => {
                self.cursor_right();
                None
            }
            KeyCode::Up => {
                self.select_prev();
                None
            }
            KeyCode::Down => {
                self.select_next();
                None
            }
            KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                if self.delete_word_backwards() {
                    self.refresh_query_words();
                }
                None
            }
            KeyCode::Char(c) => {
                let byte_pos = self
                    .query
                    .char_indices()
                    .nth(self.cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(self.query.len());
                self.query.insert(byte_pos, c);
                self.cursor_pos += 1;
                self.refresh_query_words();
                None
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0
                    && let Some((byte_pos, _)) = self.query.char_indices().nth(self.cursor_pos - 1)
                {
                    self.query.remove(byte_pos);
                    self.cursor_pos -= 1;
                    self.refresh_query_words();
                }
                None
            }
            KeyCode::Delete => {
                let len = self.query.chars().count();
                if self.cursor_pos < len
                    && let Some((byte_pos, _)) = self.query.char_indices().nth(self.cursor_pos)
                {
                    self.query.remove(byte_pos);
                    self.refresh_query_words();
                }
                None
            }
            _ => None,
        }
    }
}

/// RAII guard to ensure terminal is restored on exit
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn new() -> Result<Self> {
        terminal::enable_raw_mode().map_err(|e| AppError::Io(io::Error::other(e)))?;

        let mut stdout = io::stdout();
        if let Err(e) = crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = terminal::disable_raw_mode();
            return Err(AppError::Io(io::Error::other(e)));
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(e) => {
                let _ = terminal::disable_raw_mode();
                let _ =
                    crossterm::execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
                return Err(AppError::Io(io::Error::other(e)));
            }
        };

        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            self.terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        );
    }
}

/// Run the TUI with background loading
/// Returns the action and the final list of conversations
pub fn run_with_loader(
    rx: Receiver<LoaderMessage>,
) -> Result<(Action, Vec<Conversation>)> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = terminal::disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    let mut guard = TerminalGuard::new()?;
    let mut app = App::new_loading();

    loop {
        // Process all pending loader messages (non-blocking)
        loop {
            match rx.try_recv() {
                Ok(LoaderMessage::Fatal(err)) => {
                    drop(guard);
                    return Err(err);
                }
                Ok(LoaderMessage::ProjectError) => {}
                Ok(LoaderMessage::Batch(convs)) => {
                    app.append_conversations(convs);
                }
                Ok(LoaderMessage::Done) => {
                    app.finish_loading();
                    if app.conversations().is_empty() {
                        drop(guard);
                        return Err(AppError::NoHistoryFound("all projects".to_string()));
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    if app.is_loading() {
                        app.finish_loading();
                        if app.conversations().is_empty() {
                            drop(guard);
                            return Err(AppError::NoHistoryFound("all projects".to_string()));
                        }
                    }
                    break;
                }
            }
        }

        guard.terminal.draw(|frame| ui::render(frame, &app))?;

        if event::poll(Duration::from_millis(50)).map_err(|e| AppError::Io(io::Error::other(e)))? {
            let action = match event::read().map_err(|e| AppError::Io(io::Error::other(e)))? {
                // Only handle key press events (not release)
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key.code, key.modifiers)
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => None,
            };
            if let Some(action) = action {
                return Ok((action, app.into_conversations()));
            }
        }
    }
}
