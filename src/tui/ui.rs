use crate::search::{can_start_match, fold};
use crate::tui::app::{App, LoadingState};
use ratatui::layout::Position;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use std::borrow::Cow;

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
    let prompt_len = PROMPT.chars().count();
    let left_len = prompt_len + query.chars().count();
    let count_len = status_text.chars().count() + 1;
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
        let cursor_offset = app.cursor_pos() as u16;
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
            let list_idx = offset + relative_idx;
            let conv = &app.conversations()[conv_idx];
            let is_selected = app.selected() == Some(list_idx);

            let timestamp = conv.timestamp.format("%b %d, %H:%M").to_string();

            let msg_count = if conv.message_count == 1 {
                "1 msg".to_string()
            } else {
                format!("{} msgs", conv.message_count)
            };

            let duration = conv.duration_minutes.map(|m| {
                if m >= 60 {
                    format!("{}h {}m", m / 60, m % 60)
                } else {
                    format!("{}m", m)
                }
            });

            let indicator = " ▌ ";
            let indicator_style = Style::default().fg(if is_selected { ACCENT } else { BORDER });

            let project_part = conv.project_name.as_str();

            let duration_len = duration
                .as_ref()
                .map(|d| d.chars().count() + 3)
                .unwrap_or(0);
            let right_len =
                msg_count.chars().count() + duration_len + 3 + timestamp.chars().count();
            let indicator_len = indicator.chars().count();
            let project_len = project_part.chars().count();
            let min_padding = 2;

            let available_for_summary =
                width.saturating_sub(indicator_len + project_len + right_len + min_padding + 4);

            let summary_part = conv
                .summary
                .as_ref()
                .filter(|s| !s.is_empty() && available_for_summary > 5)
                .map(|s| format!(" · {}", truncate(s, available_for_summary)));

            let left_len = indicator_len
                + project_len
                + summary_part
                    .as_ref()
                    .map(|s| s.chars().count())
                    .unwrap_or(0);
            let padding = width.saturating_sub(left_len + right_len + 1);

            let project_style = if is_selected {
                Style::default().fg(Color::White).bold()
            } else {
                Style::default().fg(Color::White)
            };

            let summary_style = Style::default().fg(Color::Rgb(140, 155, 175));
            let summary_highlight_style = Style::default().fg(Color::Rgb(180, 195, 215));

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

            let mut header_spans = vec![Span::styled(indicator, indicator_style)];
            header_spans.extend(highlight_text(
                project_part,
                query_words,
                project_style,
                highlight_style,
            ));

            if let Some(ref summary) = summary_part {
                header_spans.extend(highlight_text(
                    summary,
                    query_words,
                    summary_style,
                    summary_highlight_style,
                ));
            }

            header_spans.push(Span::raw(" ".repeat(padding)));
            header_spans.push(Span::styled(
                msg_count,
                Style::default().fg(Color::Rgb(110, 110, 110)),
            ));
            if let Some(ref d) = duration {
                header_spans.push(Span::styled(" · ", Style::default().fg(SEPARATOR)));
                header_spans.push(Span::styled(
                    d.clone(),
                    Style::default().fg(Color::Rgb(100, 140, 130)),
                ));
            }
            header_spans.push(Span::styled(" · ", Style::default().fg(SEPARATOR)));
            header_spans.push(Span::styled(
                timestamp,
                Style::default().fg(Color::Rgb(140, 140, 140)),
            ));

            let header = Line::from(header_spans).style(selection_bg);

            // Preview line
            let truncated_preview = truncate(&conv.preview, width.saturating_sub(4));
            let preview_style = Style::default().fg(Color::Rgb(130, 130, 130));
            let mut preview_spans = vec![Span::styled(indicator, indicator_style)];
            preview_spans.extend(highlight_text(
                &truncated_preview,
                query_words,
                preview_style,
                Style::default().fg(Color::Rgb(180, 180, 180)),
            ));
            let preview = Line::from(preview_spans).style(selection_bg);

            let separator = Line::from(Span::styled(
                separator_str.as_str(),
                Style::default().fg(Color::Rgb(50, 50, 50)),
            ));

            ListItem::new(vec![header, preview, separator])
        })
        .collect();

    let list = List::new(visible_items);
    frame.render_widget(list, area);
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

/// Shorten `s` to at most `max` chars, ending with `…` when cut
fn truncate(s: &str, max: usize) -> Cow<'_, str> {
    if s.chars().count() <= max {
        return Cow::Borrowed(s);
    }
    let mut cut: String = s.chars().take(max.saturating_sub(1)).collect();
    cut.push('…');
    Cow::Owned(cut)
}
