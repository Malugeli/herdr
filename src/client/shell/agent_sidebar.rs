use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Widget},
};

use super::*;

pub(super) struct AgentRow {
    pub(super) pane_id: String,
    pub(super) status: crate::api::schema::AgentStatus,
    pub(super) focused: bool,
    pub(super) rows: Vec<Vec<crate::ui::ResolvedToken>>,
}

pub(super) fn ordered_agent_pane_ids(
    snapshot: &ClientShellSnapshot,
    sort: crate::config::AgentPanelSortConfig,
) -> Vec<String> {
    if snapshot.agent_view_label.is_some() {
        return snapshot
            .agent_order
            .iter()
            .filter(|pane_id| {
                snapshot
                    .agents
                    .iter()
                    .any(|agent| agent.pane_id == pane_id.as_str())
            })
            .cloned()
            .collect();
    }
    let mut agents = snapshot.agents.iter().collect::<Vec<_>>();
    if sort == crate::config::AgentPanelSortConfig::Priority {
        agents.sort_by_key(|agent| {
            (
                std::cmp::Reverse(status_priority(agent.agent_status)),
                std::cmp::Reverse(agent.state_change_seq),
            )
        });
    }
    agents
        .into_iter()
        .map(|agent| agent.pane_id.clone())
        .collect()
}

pub(super) fn render_agent_panel(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    agent_scroll: &mut usize,
    hits: &mut ShellHitMap,
) {
    if !render_agent_panel_header(
        buffer,
        area,
        snapshot.agent_view_label.as_deref(),
        config,
        hits,
    ) {
        return;
    }

    let rows = agent_rows(snapshot, config, None);
    render_agent_list(
        buffer,
        area,
        &rows,
        snapshot
            .agent_view_label
            .as_ref()
            .map(|_| " no matching agents"),
        config,
        agent_scroll,
        hits,
        |row| row.rows.len(),
        |buffer, rect, row, hits| {
            hits.agents.push((rect, row.pane_id.clone()));
            render_agent_row(buffer, rect, row, config);
        },
    );
}

pub(super) fn render_agent_panel_header(
    buffer: &mut Buffer,
    area: Rect,
    agent_view_label: Option<&str>,
    config: &ClientShellConfig,
    hits: &mut ShellHitMap,
) -> bool {
    if area.height == 0 {
        return false;
    }
    put_text(
        buffer,
        area.x,
        area.y,
        area.width,
        &"─".repeat(area.width as usize),
        Style::default().fg(config.palette.surface_dim),
    );
    if area.height < 2 {
        return false;
    }
    put_text(
        buffer,
        area.x,
        area.y + 1,
        area.width,
        " agents",
        Style::default()
            .fg(config.palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );
    let sort_label = agent_view_label.unwrap_or(match config.agent_panel_sort {
        crate::config::AgentPanelSortConfig::Spaces => "grouped",
        crate::config::AgentPanelSortConfig::Priority => "priority",
    });
    let sort_width = display_width(sort_label).min(area.width as usize) as u16;
    let sort_rect = Rect::new(
        area.right().saturating_sub(sort_width),
        area.y + 1,
        sort_width,
        1,
    );
    hits.agent_sort_toggle = if config.mouse_capture && agent_view_label.is_none() {
        sort_rect
    } else {
        Rect::default()
    };
    put_text(
        buffer,
        sort_rect.x,
        sort_rect.y,
        sort_rect.width,
        sort_label,
        Style::default()
            .fg(if agent_view_label.is_some() {
                config.palette.accent
            } else {
                config.palette.overlay0
            })
            .add_modifier(Modifier::BOLD),
    );
    true
}

pub(super) fn render_agent_list<T>(
    buffer: &mut Buffer,
    area: Rect,
    rows: &[T],
    empty_message: Option<&str>,
    config: &ClientShellConfig,
    agent_scroll: &mut usize,
    hits: &mut ShellHitMap,
    row_lines: impl Fn(&T) -> usize,
    mut render_row: impl FnMut(&mut Buffer, Rect, &T, &mut ShellHitMap),
) {
    let body = Rect::new(
        area.x,
        area.y.saturating_add(3),
        area.width,
        area.height.saturating_sub(3),
    );
    hits.agent_body = body;
    if body.is_empty() || rows.is_empty() {
        *agent_scroll = 0;
        if let Some(message) = empty_message.filter(|_| !body.is_empty()) {
            put_text(
                buffer,
                body.x,
                body.y,
                body.width,
                message,
                Style::default()
                    .fg(config.palette.overlay0)
                    .add_modifier(Modifier::DIM),
            );
        }
        return;
    }

    let row_heights = rows
        .iter()
        .map(|row| row_lines(row).max(1).min(u16::MAX as usize) as u16)
        .collect::<Vec<_>>();
    let gaps = rows
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if index + 1 < rows.len() {
                config.agents.row_gap
            } else {
                0
            }
        })
        .collect::<Vec<_>>();
    let metrics =
        super::scroll::list_scroll_metrics(&row_heights, &gaps, body.height, *agent_scroll);
    hits.agent_max_scroll = metrics.max_offset_from_bottom;
    hits.agent_scroll_metrics = Some(metrics);
    *agent_scroll = metrics
        .max_offset_from_bottom
        .saturating_sub(metrics.offset_from_bottom);
    let show_scrollbar = metrics.max_offset_from_bottom > 0 && body.width > 1;
    let content_width = body.width.saturating_sub(u16::from(show_scrollbar));
    let mut y = body.y;
    for (index, row) in rows.iter().enumerate().skip(*agent_scroll) {
        let height = row_heights[index].min(body.height);
        if y.saturating_add(height) > body.bottom() {
            break;
        }
        let rect = Rect::new(body.x, y, content_width, height);
        render_row(buffer, rect, row, hits);
        y = y
            .saturating_add(height)
            .saturating_add(if index + 1 < rows.len() {
                config.agents.row_gap
            } else {
                0
            });
    }

    if show_scrollbar {
        let track = Rect::new(body.right().saturating_sub(1), body.y, 1, body.height);
        hits.agent_scrollbar = track;
        super::scroll::render_list_scrollbar(buffer, track, metrics, &config.palette);
    }
}

pub(super) fn agent_rows(
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
    machine: Option<&str>,
) -> Vec<AgentRow> {
    ordered_agent_pane_ids(snapshot, config.agent_panel_sort)
        .into_iter()
        .filter_map(|pane_id| agent_row(snapshot, &pane_id, config, machine))
        .collect()
}

pub(super) fn agent_row(
    snapshot: &ClientShellSnapshot,
    pane_id: &str,
    config: &ClientShellConfig,
    machine: Option<&str>,
) -> Option<AgentRow> {
    let agent = snapshot
        .agents
        .iter()
        .find(|agent| agent.pane_id == pane_id)?;
    let workspace = snapshot
        .workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == agent.workspace_id)?;
    let tab = snapshot.tabs.iter().find(|tab| tab.tab_id == agent.tab_id);
    let pane = snapshot
        .panes
        .iter()
        .find(|pane| pane.pane_id == agent.pane_id);
    let tab_count = snapshot
        .tabs
        .iter()
        .filter(|candidate| candidate.workspace_id == agent.workspace_id)
        .count();
    let tab_label = tab
        .filter(|tab| tab_count > 1 || tab.custom_label)
        .map(|tab| tab.label.as_str());
    let agent_label = agent
        .display_agent
        .as_deref()
        .or(agent.name.as_deref())
        .or(agent.agent.as_deref())
        .or(agent.title.as_deref());
    let labels = agent
        .state_labels
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>();
    let tokens = agent.tokens.iter().cloned().collect::<HashMap<_, _>>();
    let state_text = labels
        .get(status_text(agent.agent_status))
        .map(String::as_str)
        .unwrap_or_else(|| sidebar_status_text(agent.agent_status));
    let canonical_agent = agent
        .agent
        .as_deref()
        .and_then(crate::detect::parse_agent_label);
    let rows = crate::ui::sidebar_agent_rows(
        &config.agents,
        crate::ui::AgentTokenContext {
            machine,
            workspace: &workspace.label,
            tab: tab_label,
            pane: agent
                .title
                .as_deref()
                .or_else(|| pane.and_then(|pane| pane.label.as_deref())),
            agent_label,
            terminal_title: agent.terminal_title.as_deref(),
            terminal_title_stripped: agent.terminal_title_stripped.as_deref(),
            canonical_agent,
            tokens: &tokens,
        },
        state_text,
    );
    Some(AgentRow {
        pane_id: agent.pane_id.clone(),
        status: agent.agent_status,
        focused: agent.focused,
        rows,
    })
}

pub(super) fn render_agent_row(
    buffer: &mut Buffer,
    rect: Rect,
    row: &AgentRow,
    config: &ClientShellConfig,
) {
    let palette = &config.palette;
    let row_style = if row.focused {
        Style::default().bg(palette.active_row_bg)
    } else {
        Style::default()
    };
    let name_style = if row.focused {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(palette.subtext0)
            .add_modifier(Modifier::BOLD)
    };
    let status_style = Style::default().fg(status_color(row.status, palette));
    let secondary = Style::default().fg(palette.overlay0);
    let icon = (
        status_icon(row.status, config.status_indicators),
        Style::default().fg(status_color(row.status, palette)),
    );
    let rows = if row.rows.is_empty() {
        vec![vec![crate::ui::ResolvedToken {
            kind: crate::ui::ResolvedTokenKind::StateIcon,
            style: Default::default(),
        }]]
    } else {
        row.rows.clone()
    };
    for (index, tokens) in rows.iter().take(rect.height as usize).enumerate() {
        let indent = if index == 0 { 1 } else { 3 };
        let mut spans = vec![ratatui::text::Span::raw(" ".repeat(indent))];
        spans.extend(crate::ui::resolved_token_spans(
            tokens,
            icon,
            status_style,
            name_style,
            secondary,
            secondary,
            palette,
            rect.width.saturating_sub(indent as u16) as usize,
        ));
        Paragraph::new(Line::from(spans)).style(row_style).render(
            Rect::new(rect.x, rect.y + index as u16, rect.width, 1),
            buffer,
        );
    }
}

/// Rows reserved above the Current Space section body: divider, heading, blank.
const CURRENT_SPACE_HEADER_ROWS: u16 = 3;
/// Rows the Agents section keeps below the Current Space section: header plus two lines.
const AGENTS_SECTION_MIN_ROWS: u16 = 5;

pub(super) struct CurrentSpaceRow {
    /// One-based tab position within the Space, the digit for `prefix+N`.
    pub(super) tab_number: usize,
    pub(super) row: AgentRow,
}

/// Agents of the focused Space in tab order, keeping pane order within a tab.
pub(super) fn current_space_rows(
    snapshot: &ClientShellSnapshot,
    config: &ClientShellConfig,
) -> Vec<CurrentSpaceRow> {
    let Some(workspace_id) = snapshot.focused_workspace_id.as_deref() else {
        return Vec::new();
    };
    let mut rows = snapshot
        .agents
        .iter()
        .filter(|agent| agent.workspace_id == workspace_id)
        .filter_map(|agent| {
            // `prefix+N` focuses the Nth tab of the Space in snapshot order; `tab.number` is not that.
            let tab_number = snapshot
                .tabs
                .iter()
                .filter(|tab| tab.workspace_id == workspace_id)
                .position(|tab| tab.tab_id == agent.tab_id)?
                + 1;
            let mut row = agent_row(snapshot, &agent.pane_id, config, None)?;
            // Every row belongs to the Current Space, so its name would only repeat the heading.
            for line in &mut row.rows {
                line.retain(|token| {
                    !matches!(token.kind, crate::ui::ResolvedTokenKind::Workspace(_))
                });
            }
            row.rows.retain(|line| !line.is_empty());
            Some(CurrentSpaceRow { tab_number, row })
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.tab_number);
    rows
}

/// Scroll position of the Current Space section, kept across frames.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CurrentSpaceScroll {
    /// Index of the first visible row.
    pub(super) offset: usize,
    /// Focused pane the offset was last revealed for; a new focus scrolls it into view once.
    revealed_pane_id: Option<String>,
}

/// Fixed height of the Current Space section carved from the top of `detail_area`, or 0 when
/// there is no room. It ignores the number of agents so switching Spaces never moves the
/// sections below.
pub(super) fn current_space_section_height(sidebar_height: u16, detail_height: u16) -> u16 {
    let height = (sidebar_height / 3).min(detail_height.saturating_sub(AGENTS_SECTION_MIN_ROWS));
    if height <= CURRENT_SPACE_HEADER_ROWS {
        return 0;
    }
    height
}

fn current_space_row_height(row: &CurrentSpaceRow) -> usize {
    row.row.rows.len().max(1)
}

/// Rows from `start` that fit in `height` lines, counting gaps only between visible rows.
fn current_space_rows_fitting(
    rows: &[CurrentSpaceRow],
    start: usize,
    height: usize,
    gap: usize,
) -> usize {
    let mut used = 0;
    let mut count = 0;
    for row in rows.iter().skip(start) {
        let needed = current_space_row_height(row) + if count > 0 { gap } else { 0 };
        if used + needed > height {
            break;
        }
        used += needed;
        count += 1;
    }
    count
}

/// Rows visible from `start`, leaving the last line for "+N more" when rows remain below.
fn current_space_visible_rows(
    rows: &[CurrentSpaceRow],
    start: usize,
    height: usize,
    gap: usize,
) -> usize {
    let all = current_space_rows_fitting(rows, start, height, gap);
    if start + all >= rows.len() {
        return all;
    }
    current_space_rows_fitting(rows, start, height.saturating_sub(1), gap)
}

/// Smallest offset whose visible rows reach the last row, so scrolling stops there.
fn current_space_max_offset(rows: &[CurrentSpaceRow], height: usize, gap: usize) -> usize {
    (0..rows.len())
        .find(|start| start + current_space_visible_rows(rows, *start, height, gap) >= rows.len())
        .unwrap_or(0)
}

pub(super) fn render_current_space_section(
    buffer: &mut Buffer,
    area: Rect,
    snapshot: &ClientShellSnapshot,
    rows: &[CurrentSpaceRow],
    config: &ClientShellConfig,
    scroll: &mut CurrentSpaceScroll,
    hits: &mut ShellHitMap,
) {
    if area.height < CURRENT_SPACE_HEADER_ROWS {
        return;
    }
    let palette = &config.palette;
    put_text(
        buffer,
        area.x,
        area.y,
        area.width,
        &"─".repeat(area.width as usize),
        Style::default().fg(palette.surface_dim),
    );
    let label = snapshot
        .focused_workspace_id
        .as_deref()
        .and_then(|id| {
            snapshot
                .workspaces
                .iter()
                .find(|workspace| workspace.workspace_id == id)
        })
        .map(|workspace| workspace.label.as_str())
        .unwrap_or_default();
    put_text(
        buffer,
        area.x,
        area.y + 1,
        area.width,
        &format!(" {label}"),
        Style::default()
            .fg(palette.overlay0)
            .add_modifier(Modifier::BOLD),
    );

    let body = Rect::new(
        area.x,
        area.y + CURRENT_SPACE_HEADER_ROWS,
        area.width,
        area.height - CURRENT_SPACE_HEADER_ROWS,
    );
    hits.current_space_body = body;
    if body.is_empty() {
        return;
    }
    if rows.is_empty() {
        scroll.offset = 0;
        put_text(
            buffer,
            body.x,
            body.y,
            body.width,
            " no agents",
            Style::default()
                .fg(palette.overlay0)
                .add_modifier(Modifier::DIM),
        );
        return;
    }

    let height = body.height as usize;
    let gap = config.agents.row_gap as usize;
    let max_offset = current_space_max_offset(rows, height, gap);
    hits.current_space_max_scroll = max_offset;
    let focused = rows.iter().position(|entry| entry.row.focused);
    let focused_pane_id = focused.map(|index| rows[index].row.pane_id.clone());
    if let Some(index) = focused.filter(|_| focused_pane_id != scroll.revealed_pane_id) {
        if index < scroll.offset {
            scroll.offset = index;
        }
        while scroll.offset < index
            && scroll.offset + current_space_visible_rows(rows, scroll.offset, height, gap) <= index
        {
            scroll.offset += 1;
        }
        scroll.revealed_pane_id = focused_pane_id;
    }
    scroll.offset = scroll.offset.min(max_offset);

    let visible = current_space_visible_rows(rows, scroll.offset, height, gap);
    let number_width = rows
        .iter()
        .map(|row| row.tab_number.to_string().len())
        .max()
        .unwrap_or(1);
    let mut y = body.y;
    for (index, entry) in rows.iter().enumerate().skip(scroll.offset).take(visible) {
        if index > scroll.offset {
            y = y.saturating_add(config.agents.row_gap);
        }
        let row_height = current_space_row_height(entry) as u16;
        let rect = Rect::new(body.x, y, body.width, row_height);
        hits.current_space_agents
            .push((rect, entry.row.pane_id.clone()));
        render_current_space_row(buffer, rect, entry, number_width, config);
        y = y.saturating_add(row_height);
    }
    let hidden_below = rows.len() - scroll.offset - visible;
    if hidden_below > 0 {
        put_text(
            buffer,
            body.x,
            body.bottom() - 1,
            body.width,
            &format!(" +{hidden_below} more"),
            Style::default()
                .fg(palette.overlay0)
                .add_modifier(Modifier::DIM),
        );
    }
}

fn render_current_space_row(
    buffer: &mut Buffer,
    rect: Rect,
    entry: &CurrentSpaceRow,
    number_width: usize,
    config: &ClientShellConfig,
) {
    let row = &entry.row;
    let palette = &config.palette;
    let row_style = if row.focused {
        Style::default().bg(palette.active_row_bg)
    } else {
        Style::default()
    };
    let name_style = if row.focused {
        Style::default()
            .fg(palette.text)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(palette.subtext0)
            .add_modifier(Modifier::BOLD)
    };
    let status_style = Style::default().fg(status_color(row.status, palette));
    let secondary = Style::default().fg(palette.overlay0);
    let icon = (
        status_icon(row.status, config.status_indicators),
        status_style,
    );
    let lines = if row.rows.is_empty() {
        vec![vec![crate::ui::ResolvedToken {
            kind: crate::ui::ResolvedTokenKind::StateIcon,
            style: Default::default(),
        }]]
    } else {
        row.rows.clone()
    };
    // " N " before the first line; continuation lines align under its tokens.
    let prefix_width = number_width + 2;
    for (index, tokens) in lines.iter().take(rect.height as usize).enumerate() {
        let mut spans = if index == 0 {
            vec![ratatui::text::Span::styled(
                format!(" {:>number_width$} ", entry.tab_number),
                if row.focused {
                    Style::default().fg(palette.text)
                } else {
                    secondary
                },
            )]
        } else {
            vec![ratatui::text::Span::raw(" ".repeat(prefix_width + 2))]
        };
        let used = if index == 0 {
            prefix_width
        } else {
            prefix_width + 2
        };
        spans.extend(crate::ui::resolved_token_spans(
            tokens,
            icon,
            status_style,
            name_style,
            secondary,
            secondary,
            palette,
            (rect.width as usize).saturating_sub(used),
        ));
        Paragraph::new(Line::from(spans)).style(row_style).render(
            Rect::new(rect.x, rect.y + index as u16, rect.width, 1),
            buffer,
        );
    }
}

fn put_text(buffer: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) {
    for (offset, character) in text.chars().take(width as usize).enumerate() {
        if let Some(cell) = buffer.cell_mut((x + offset as u16, y)) {
            cell.set_char(character).set_style(style);
        }
    }
}

fn display_width(text: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(text)
}

fn sidebar_status_text(status: crate::api::schema::AgentStatus) -> &'static str {
    use crate::api::schema::AgentStatus;
    match status {
        AgentStatus::Blocked => "blocked",
        AgentStatus::Done => "done",
        AgentStatus::Working => "working",
        AgentStatus::Idle | AgentStatus::Unknown => "idle",
    }
}
