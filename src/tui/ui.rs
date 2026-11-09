use crate::history::Conversation;
use crate::search::{can_start_match, fold};
use crate::tui::app::{App, LoadingState};
use ratatui::layout::Position;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use std::borrow::Cow;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Lines per conversation item (header + preview + separator)
pub const LINES_PER_ITEM: usize = 3;

const ACCENT: Color = Color::Rgb(78, 201, 176);
const BORDER: Color = Color::Rgb(60, 60, 60);
const LABEL: Color = Color::Rgb(100, 100, 100);
const SEPARATOR: Color = Color::Rgb(70, 70, 70);

/// Search prompt; the query is drawn right after it
const PROMPT: &str = " ❯ ";

/// Index of the first item on the page that holds `selected`
pub fn page_offset(selected: Option<usize>, items_per_page: usize) -> usize {
    match (selected, items_per_page) {
        (Some(sel), n) if n > 0 => sel / n * n,
        _ => 0,
    }
}

/// Render the TUI
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER));
    let inner_area = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    let show_status_bar = inner_area.height >= 4;
    let mut constraints = vec![Constraint::Length(2), Constraint::Min(1)];
    if show_status_bar {
        constraints.push(Constraint::Length(1));
    }
    let chunks = Layout::vertical(constraints).split(inner_area);

    render_search_bar(frame, app, chunks[0]);
    render_list(frame, app, chunks[1]);
    if show_status_bar {
        render_status_bar(frame, app, chunks[2]);
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let key_style = Style::default().fg(ACCENT);
    let label_style = Style::default().fg(LABEL);
    let dim_style = Style::default().fg(BORDER);

    let (action_key, action_label) = if app.is_loading() {
        (dim_style, dim_style)
    } else {
        (key_style, label_style)
    };

    let spans = if app.is_hide_pending() {
        let warn_style = Style::default().fg(Color::Rgb(230, 110, 110));
        vec![
            Span::raw("  "),
            Span::styled("Ctrl+D", warn_style.bold()),
            Span::styled(" again to hide this conversation  ", warn_style),
            Span::styled("any key", key_style),
            Span::styled(" cancel", label_style),
        ]
    } else {
        vec![
            Span::raw("  "),
            Span::styled("Enter", action_key),
            Span::styled(" resume  ", action_label),
            Span::styled("Ctrl+D", action_key),
            Span::styled(" hide  ", action_label),
            Span::styled("Esc", key_style),
            Span::styled(" quit", label_style),
        ]
    };

    let status_line = Line::from(spans);
    let status = Paragraph::new(status_line).style(Style::default().bg(Color::Rgb(30, 30, 35)));
    frame.render_widget(status, area);
}

fn render_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status_text = match app.loading_state() {
        LoadingState::Loading { loaded } => {
            format!("Loading... {}", loaded)
        }
        LoadingState::Ready => {
            format!("{}/{}", app.filtered().len(), app.conversations().len())
        }
    };

    let query = app.query();
    let prompt_len = text_width(PROMPT);
    let left_len = prompt_len + text_width(query);
    let count_len = text_width(&status_text) + 1;
    let padding = (area.width as usize).saturating_sub(left_len + count_len + 1);

    let status_style = Style::default().fg(if app.is_loading() { ACCENT } else { LABEL });

    let search_line = Line::from(vec![
        Span::styled(PROMPT, Style::default().fg(ACCENT)),
        Span::raw(query),
        Span::raw(" ".repeat(padding)),
        Span::styled(status_text, status_style),
        Span::raw(" "),
    ]);

    let input = Paragraph::new(search_line).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(BORDER)),
    );

    frame.render_widget(input, area);

    if area.width as usize > prompt_len {
        let before_cursor: String = query.chars().take(app.cursor_pos()).collect();
        let cursor_offset = text_width(&before_cursor) as u16;
        let max_x = area.x + area.width.saturating_sub(2);
        let cursor_x = (area.x + prompt_len as u16)
            .saturating_add(cursor_offset)
            .min(max_x);
        frame.set_cursor_position(Position::new(cursor_x, area.y));
    }
}

fn render_list(frame: &mut Frame, app: &App, area: Rect) {
    app.set_list_area(area);
    let width = area.width as usize;
    let query_words = app.query_words();

    let items_per_page = (area.height as usize) / LINES_PER_ITEM;
    let offset = page_offset(app.selected(), items_per_page);
    let visible_count = items_per_page.max(1);

    let separator_str = "─".repeat(width);

    let visible_items: Vec<ListItem> = app
        .filtered()
        .iter()
        .skip(offset)
        .take(visible_count)
        .enumerate()
        .map(|(relative_idx, &conv_idx)| {
            let conv = &app.conversations()[conv_idx];
            let is_selected = app.selected() == Some(offset + relative_idx);
            render_item(conv, is_selected, width, query_words, &separator_str)
        })
        .collect();

    let list = List::new(visible_items);
    frame.render_widget(list, area);
}

/// Room the title keeps before metadata on the right starts being dropped
const TITLE_ROOM: usize = 24;
const META_SEPARATOR: &str = " · ";

/// A piece of header metadata; when the line is too narrow the piece with
/// the lowest `priority` is dropped first
struct Meta {
    text: String,
    style: Style,
    priority: u8,
}

fn render_item<'a>(
    conv: &Conversation,
    is_selected: bool,
    width: usize,
    query_words: &[String],
    separator: &'a str,
) -> ListItem<'a> {
    let indicator = " ▌ ";
    let indicator_style = Style::default().fg(if is_selected { ACCENT } else { BORDER });
    let highlight_style = if is_selected {
        Style::default().fg(ACCENT).bold()
    } else {
        Style::default().fg(ACCENT)
    };
    let selection_bg = if is_selected {
        Style::default().bg(Color::Rgb(45, 45, 55))
    } else {
        Style::default()
    };

    let mut meta = Vec::new();
    let mut add = |text: String, color: Color, priority: u8| {
        meta.push(Meta {
            text,
            style: Style::default().fg(color),
            priority,
        })
    };
    if conv.continued_in.is_some() {
        add("↪ continued".into(), Color::Rgb(200, 160, 90), 6);
    }
    if conv.is_sdk {
        add("sdk".into(), Color::Rgb(120, 120, 150), 2);
    }
    if let Some(model) = &conv.model {
        add(model.clone(), Color::Rgb(150, 130, 190), 1);
    }
    if conv.lines_added + conv.lines_removed > 0 {
        let lines = format!("+{} -{}", conv.lines_added, conv.lines_removed);
        add(lines, Color::Rgb(110, 150, 120), 0);
    }
    let msg_count = match conv.message_count {
        1 => "1 msg".to_string(),
        n => format!("{n} msgs"),
    };
    add(msg_count, Color::Rgb(110, 110, 110), 9);
    if let Some(m) = conv.duration_minutes {
        let duration = if m >= 60 {
            format!("{}h {}m", m / 60, m % 60)
        } else {
            format!("{m}m")
        };
        add(duration, Color::Rgb(100, 140, 130), 4);
    }
    let timestamp = conv.timestamp.format("%b %d, %H:%M").to_string();
    add(timestamp, Color::Rgb(140, 140, 140), 10);

    let project = conv.project_name.as_str();
    let branch = conv.git_branch.as_ref().map(|b| format!("@{b}"));
    let left_base =
        text_width(indicator) + text_width(project) + branch.as_ref().map_or(0, |b| text_width(b));
    let meta_len = |meta: &[Meta]| {
        meta.iter().map(|m| text_width(&m.text)).sum::<usize>()
            + text_width(META_SEPARATOR) * meta.len().saturating_sub(1)
    };
    let title_room = if conv.title.is_some() { TITLE_ROOM } else { 0 };
    // Message count and time always stay
    while meta.len() > 2 && left_base + meta_len(&meta) + title_room + 2 > width {
        let lowest = meta
            .iter()
            .enumerate()
            .min_by_key(|(_, m)| m.priority)
            .map(|(i, _)| i);
        if let Some(i) = lowest {
            meta.remove(i);
        }
    }
    let right_len = meta_len(&meta);

    let available_for_title = width.saturating_sub(left_base + right_len + 6);
    let title_part = conv
        .title
        .as_ref()
        .filter(|s| !s.is_empty() && available_for_title > 5)
        .map(|s| format!("{META_SEPARATOR}{}", truncate(s, available_for_title)));

    let left_len = left_base + title_part.as_ref().map_or(0, |s| text_width(s));
    let padding = width.saturating_sub(left_len + right_len + 1);

    let project_style = if is_selected {
        Style::default().fg(Color::White).bold()
    } else {
        Style::default().fg(Color::White)
    };

    let mut header_spans = vec![Span::styled(indicator, indicator_style)];
    header_spans.extend(highlight_text(
        project,
        query_words,
        project_style,
        highlight_style,
    ));
    if let Some(branch) = &branch {
        header_spans.extend(highlight_text(
            branch,
            query_words,
            Style::default().fg(Color::Rgb(110, 130, 150)),
            highlight_style,
        ));
    }
    if let Some(title) = &title_part {
        header_spans.extend(highlight_text(
            title,
            query_words,
            Style::default().fg(Color::Rgb(140, 155, 175)),
            Style::default().fg(Color::Rgb(180, 195, 215)),
        ));
    }
    header_spans.push(Span::raw(" ".repeat(padding)));
    for (i, m) in meta.into_iter().enumerate() {
        if i > 0 {
            header_spans.push(Span::styled(META_SEPARATOR, Style::default().fg(SEPARATOR)));
        }
        header_spans.push(Span::styled(m.text, m.style));
    }
    let header = Line::from(header_spans).style(selection_bg);

    // Preview line: the recap if there is one, else the first messages and
    // the latest prompt
    let available = width.saturating_sub(4);
    let preview_style = Style::default().fg(Color::Rgb(130, 130, 130));
    let preview_highlight = Style::default().fg(Color::Rgb(180, 180, 180));
    let mut preview_spans = vec![Span::styled(indicator, indicator_style)];
    let last_prompt = conv
        .last_prompt
        .as_ref()
        .filter(|p| !conv.preview.starts_with(p.as_str()));
    if let Some(recap) = &conv.recap {
        // Told apart from messages by its color alone
        preview_spans.extend(highlight_text(
            &truncate(recap, available),
            query_words,
            Style::default().fg(Color::Rgb(150, 165, 160)),
            preview_highlight,
        ));
    } else if let Some(last) = last_prompt {
        const ARROW: &str = " » ";
        let arrow_len = text_width(ARROW);
        let first_len = (available / 2).max(available.saturating_sub(text_width(last) + arrow_len));
        preview_spans.extend(highlight_text(
            &truncate(&conv.preview, first_len),
            query_words,
            preview_style,
            preview_highlight,
        ));
        preview_spans.push(Span::styled(ARROW, Style::default().fg(ACCENT)));
        preview_spans.extend(highlight_text(
            &truncate(last, available.saturating_sub(first_len + arrow_len)),
            query_words,
            Style::default().fg(Color::Rgb(160, 160, 160)),
            preview_highlight,
        ));
    } else {
        preview_spans.extend(highlight_text(
            &truncate(&conv.preview, available),
            query_words,
            preview_style,
            preview_highlight,
        ));
    }
    let preview = Line::from(preview_spans).style(selection_bg);

    let separator = Line::from(Span::styled(
        separator,
        Style::default().fg(Color::Rgb(50, 50, 50)),
    ));

    ListItem::new(vec![header, preview, separator])
}

/// Split text into spans with matched portions highlighted (case-insensitive)
fn highlight_text(
    text: &str,
    query_words: &[String],
    base_style: Style,
    highlight_style: Style,
) -> Vec<Span<'static>> {
    if query_words.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let chars: Vec<char> = text.chars().collect();
    // `fold` maps one char to one char, so indices line up with `chars`
    let folded: Vec<char> = chars.iter().copied().map(fold).collect();
    let mut queries: Vec<Vec<char>> = query_words
        .iter()
        .map(|qw| qw.chars().collect::<Vec<_>>())
        .collect();
    // Prefer the longest query word when several match at one position
    queries.sort_by_key(|qw| std::cmp::Reverse(qw.len()));

    let char_to_byte: Vec<usize> = text
        .char_indices()
        .map(|(byte_idx, _)| byte_idx)
        .chain(std::iter::once(text.len()))
        .collect();

    let mut spans = Vec::new();
    let mut last_end = 0;
    let mut char_idx = 0;

    while char_idx < chars.len() {
        let prev = char_idx.checked_sub(1).map(|i| chars[i]);
        let matched = queries
            .iter()
            .find(|qw| folded[char_idx..].starts_with(qw) && can_start_match(prev, qw[0]));

        let Some(qw) = matched else {
            char_idx += 1;
            continue;
        };

        let end = char_idx + qw.len();
        if char_idx > last_end {
            spans.push(Span::styled(
                text[char_to_byte[last_end]..char_to_byte[char_idx]].to_string(),
                base_style,
            ));
        }
        spans.push(Span::styled(
            text[char_to_byte[char_idx]..char_to_byte[end]].to_string(),
            highlight_style,
        ));
        last_end = end;
        char_idx = end;
    }

    if last_end < chars.len() {
        let start_byte = char_to_byte[last_end];
        spans.push(Span::styled(text[start_byte..].to_string(), base_style));
    }

    if spans.is_empty() {
        vec![Span::styled(text.to_string(), base_style)]
    } else {
        spans
    }
}

/// Terminal cells `s` takes up (CJK characters take two)
fn text_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

/// Shorten `s` to at most `max` terminal cells, ending with `…` when cut
fn truncate(s: &str, max: usize) -> Cow<'_, str> {
    if text_width(s) <= max {
        return Cow::Borrowed(s);
    }
    let budget = max.saturating_sub(1);
    let mut cut = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > budget {
            break;
        }
        used += w;
        cut.push(c);
    }
    cut.push('…');
    Cow::Owned(cut)
}
