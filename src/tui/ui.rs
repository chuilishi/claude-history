use crate::tui::app::{App, LoadingState};
use crate::tui::search::is_word_separator;
use ratatui::layout::Position;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};

/// Lines per conversation item (header + preview + separator)
const LINES_PER_ITEM: usize = 3;

/// Render the TUI
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(60, 60, 60)));
    let inner_area = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    if inner_area.height < 4 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(inner_area);
        render_search_bar(frame, app, chunks[0]);
        render_list(frame, app, chunks[1]);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner_area);

    render_search_bar(frame, app, chunks[0]);
    render_list(frame, app, chunks[1]);
    render_status_bar(frame, app, chunks[2]);
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let is_loading = app.is_loading();

    let key_style = Style::default().fg(Color::Rgb(78, 201, 176));
    let label_style = Style::default().fg(Color::Rgb(100, 100, 100));
    let dim_key_style = Style::default().fg(Color::Rgb(60, 60, 60));
    let dim_label_style = Style::default().fg(Color::Rgb(60, 60, 60));

    let (action_key, action_label) = if is_loading {
        (dim_key_style, dim_label_style)
    } else {
        (key_style, label_style)
    };

    let spans = vec![
        Span::raw("  "),
        Span::styled("Enter", action_key),
        Span::styled(" resume  ", action_label),
        Span::styled("Esc", key_style),
        Span::styled(" quit", label_style),
    ];

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

    let prompt = " ❯ ";
    let query = app.query();
    let left_len = prompt.chars().count() + query.chars().count();
    let count_len = status_text.chars().count() + 1;
    let padding = (area.width as usize).saturating_sub(left_len + count_len + 1);

    let prompt_style = Style::default().fg(Color::Rgb(78, 201, 176));

    let status_style = if app.is_loading() {
        Style::default().fg(Color::Rgb(78, 201, 176))
    } else {
        Style::default().fg(Color::Rgb(100, 100, 100))
    };

    let search_line = Line::from(vec![
        Span::raw(" "),
        Span::styled("❯ ", prompt_style),
        Span::raw(query.to_string()),
        Span::raw(" ".repeat(padding)),
        Span::styled(status_text, status_style),
        Span::raw(" "),
    ]);

    let input = Paragraph::new(search_line).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Rgb(60, 60, 60))),
    );

    frame.render_widget(input, area);

    if area.width > 3 {
        let cursor_offset = app.cursor_pos() as u16;
        let max_x = area.x + area.width.saturating_sub(2);
        let cursor_x = (area.x + 3).saturating_add(cursor_offset).min(max_x);
        frame.set_cursor_position(Position::new(cursor_x, area.y));
    }
}

fn render_list(frame: &mut Frame, app: &App, area: Rect) {
    let width = area.width as usize;
    let query_words: Vec<&str> = app.query_words().iter().map(|s| s.as_str()).collect();

    let items_per_page = (area.height as usize) / LINES_PER_ITEM;
    let offset = match (app.selected(), items_per_page) {
        (Some(sel), n) if n > 0 => (sel / n) * n,
        _ => 0,
    };
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
            let indicator_style = if is_selected {
                Style::default().fg(Color::Rgb(78, 201, 176))
            } else {
                Style::default().fg(Color::Rgb(60, 60, 60))
            };

            let project_part = conv
                .project_name
                .as_ref()
                .map(|name| name.to_string())
                .unwrap_or_default();

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
                .map(|s| {
                    let summary_chars = s.chars().count();
                    if summary_chars > available_for_summary {
                        format!(
                            " · {}…",
                            s.chars()
                                .take(available_for_summary.saturating_sub(1))
                                .collect::<String>()
                        )
                    } else {
                        format!(" · {}", s)
                    }
                });

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
                Style::default().fg(Color::Rgb(78, 201, 176)).bold()
            } else {
                Style::default().fg(Color::Rgb(78, 201, 176))
            };

            let selection_bg = if is_selected {
                Style::default().bg(Color::Rgb(45, 45, 55))
            } else {
                Style::default()
            };

            let mut header_spans = vec![Span::styled(indicator, indicator_style)];
            header_spans.extend(highlight_text(
                &project_part,
                &query_words,
                project_style,
                highlight_style,
            ));

            if let Some(ref summary) = summary_part {
                header_spans.extend(highlight_text(
                    summary,
                    &query_words,
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
                header_spans.push(Span::styled(
                    " · ",
                    Style::default().fg(Color::Rgb(70, 70, 70)),
                ));
                header_spans.push(Span::styled(
                    d.clone(),
                    Style::default().fg(Color::Rgb(100, 140, 130)),
                ));
            }
            header_spans.push(Span::styled(
                " · ",
                Style::default().fg(Color::Rgb(70, 70, 70)),
            ));
            header_spans.push(Span::styled(
                timestamp,
                Style::default().fg(Color::Rgb(140, 140, 140)),
            ));

            let header = Line::from(header_spans).style(selection_bg);

            // Preview line
            let preview_text = sanitize_preview(&conv.preview);
            let max_preview_len = width.saturating_sub(4);
            let truncated_preview = if preview_text.chars().count() > max_preview_len {
                let truncated: String = preview_text
                    .chars()
                    .take(max_preview_len.saturating_sub(1))
                    .collect();
                format!("{}…", truncated)
            } else {
                preview_text
            };
            let preview_style = Style::default().fg(Color::Rgb(130, 130, 130));
            let mut preview_spans = vec![Span::styled(indicator, indicator_style)];
            preview_spans.extend(highlight_text(
                &truncated_preview,
                &query_words,
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
    query_words: &[&str],
    base_style: Style,
    highlight_style: Style,
) -> Vec<Span<'static>> {
    if query_words.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let chars: Vec<char> = text.chars().collect();

    let char_to_byte: Vec<usize> = text
        .char_indices()
        .map(|(byte_idx, _)| byte_idx)
        .chain(std::iter::once(text.len()))
        .collect();

    let mut spans = Vec::new();
    let mut last_end = 0;
    let mut char_idx = 0;

    while char_idx < chars.len() {
        let at_word_start = if char_idx == 0 {
            !is_word_separator(chars[char_idx])
        } else {
            is_word_separator(chars[char_idx - 1]) && !is_word_separator(chars[char_idx])
        };

        if at_word_start {
            let word_start = char_idx;
            let mut word_end = char_idx;
            while word_end < chars.len() && !is_word_separator(chars[word_end]) {
                word_end += 1;
            }

            let start_byte = char_to_byte[word_start];
            let end_byte = char_to_byte[word_end];
            let original_word = &text[start_byte..end_byte];
            let word_lower = original_word.to_lowercase();

            let matched_query = query_words.iter().find(|&&qw| word_lower.starts_with(qw));

            if let Some(qw) = matched_query {
                if word_start > last_end {
                    let prev_start_byte = char_to_byte[last_end];
                    spans.push(Span::styled(
                        text[prev_start_byte..start_byte].to_string(),
                        base_style,
                    ));
                }

                let prefix_len = qw.chars().count();
                let highlight_end = (word_start + prefix_len).min(word_end);
                let highlight_end_byte = char_to_byte[highlight_end];
                spans.push(Span::styled(
                    text[start_byte..highlight_end_byte].to_string(),
                    highlight_style,
                ));

                if highlight_end < word_end {
                    spans.push(Span::styled(
                        text[highlight_end_byte..end_byte].to_string(),
                        base_style,
                    ));
                }

                last_end = word_end;
            }
            char_idx = word_end;
        } else {
            char_idx += 1;
        }
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

/// Remove XML-like tags from preview text
fn sanitize_preview(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}
