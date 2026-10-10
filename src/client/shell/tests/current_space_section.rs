use super::*;

/// Tall enough that the Agents section below the fixed-height section shows every fixture agent.
const FRAME_ROWS: u16 = 60;

fn agent(pane_id: &str, workspace_id: &str, tab_id: &str, name: &str) -> ClientShellAgent {
    ClientShellAgent {
        pane_id: pane_id.into(),
        workspace_id: workspace_id.into(),
        tab_id: tab_id.into(),
        name: Some(name.into()),
        display_agent: None,
        agent: Some("claude".into()),
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        agent_status: AgentStatus::Idle,
        state_change_seq: 1,
        state_labels: Vec::new(),
        tokens: Vec::new(),
        focused: false,
    }
}

/// `number` is a stable id that drifts from the tab's position once tabs are closed.
fn tab(tab_id: &str, workspace_id: &str, number: usize) -> ClientShellTab {
    ClientShellTab {
        tab_id: tab_id.into(),
        workspace_id: workspace_id.into(),
        number,
        label: number.to_string(),
        custom_label: false,
        zoomed: false,
        focused: tab_id == "tab_1",
        agent_status: AgentStatus::Idle,
    }
}

/// `aviv` (focused) has agents at tab positions 3 and 1 and a plain shell at 2; `thegoal` has one agent.
fn two_spaces() -> ClientShellSnapshot {
    let mut projected = snapshot();
    projected.workspaces[0].label = "aviv".into();
    let mut other = projected.workspaces[0].clone();
    other.workspace_id = "ws_2".into();
    other.number = 2;
    other.label = "thegoal".into();
    other.focused = false;
    projected.workspaces.push(other);
    projected.tabs = vec![
        tab("tab_1", "ws_1", 79),
        tab("tab_4", "ws_2", 1),
        tab("tab_2", "ws_1", 84),
        tab("tab_3", "ws_1", 85),
    ];
    projected.agents = vec![
        agent("pane_codex", "ws_1", "tab_3", "codex-agent"),
        agent("pane_other", "ws_2", "tab_4", "other-agent"),
        agent("pane_claude", "ws_1", "tab_1", "claude-agent"),
    ];
    projected
}

fn compose(projected: ClientShellSnapshot, enabled: bool) -> (ClientShellState, String) {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.config.current_space_section = enabled;
    state.set_snapshot(Box::new(projected));
    state.set_pane_surface(surface());
    let frame = state.compose(106, FRAME_ROWS).expect("sidebar frame");
    let text = frame
        .cells
        .chunks(frame.width as usize)
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");
    (state, text)
}

fn line_at(text: &str, rect: Rect) -> String {
    text.lines()
        .nth(rect.y as usize)
        .unwrap_or_default()
        .chars()
        .skip(rect.x as usize)
        .take(rect.width as usize)
        .collect()
}

#[test]
fn lists_current_space_agents_in_tab_order_with_tab_numbers() {
    let (state, text) = compose(two_spaces(), true);

    let current = state
        .hits
        .current_space_agents
        .iter()
        .map(|(_, pane_id)| pane_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(current, ["pane_claude", "pane_codex"]);

    let first = line_at(&text, state.hits.current_space_agents[0].0);
    let second = line_at(&text, state.hits.current_space_agents[1].0);
    assert!(first.starts_with(" 1 "), "row: {first:?}");
    assert!(second.starts_with(" 3 "), "row: {second:?}");
    assert!(!first.contains("aviv"), "row repeats the space: {first:?}");

    let heading = state.hits.current_space_agents[0].0.y - 2;
    let heading_line = text.lines().nth(heading as usize).unwrap_or_default();
    assert!(
        heading_line.trim_start().starts_with("aviv"),
        "frame: {text}"
    );
}

#[test]
fn agents_section_still_lists_every_space() {
    let (state, _) = compose(two_spaces(), true);

    let mut all = state
        .hits
        .agents
        .iter()
        .map(|(_, pane_id)| pane_id.as_str())
        .collect::<Vec<_>>();
    all.sort_unstable();
    assert_eq!(all, ["pane_claude", "pane_codex", "pane_other"]);
    let section_bottom = state
        .hits
        .current_space_agents
        .iter()
        .map(|(rect, _)| rect.bottom())
        .max()
        .expect("current space rows");
    assert!(state
        .hits
        .agents
        .iter()
        .all(|(rect, _)| rect.y >= section_bottom));
}

#[test]
fn empty_current_space_shows_placeholder() {
    let mut projected = two_spaces();
    projected
        .agents
        .retain(|agent| agent.workspace_id != "ws_1");
    let (state, text) = compose(projected, true);

    assert!(state.hits.current_space_agents.is_empty());
    assert!(text.contains(" no agents"), "frame: {text}");
    assert_eq!(state.hits.agents.len(), 1);
}

#[test]
fn clicking_a_row_focuses_its_pane() {
    let (mut state, _) = compose(two_spaces(), true);
    let (rect, _) = state.hits.current_space_agents[1].clone();

    let click = state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x + 4,
        row: rect.y,
        modifiers: KeyModifiers::empty(),
    })]);
    let [ClientShellAction::Endpoint { request, .. }] = &click.actions[..] else {
        panic!("row should focus through endpoint API");
    };
    assert!(matches!(
        &request.method,
        crate::api::schema::Method::PaneFocus(target) if target.pane_id == "pane_codex"
    ));
}

#[test]
fn disabled_section_restores_two_section_layout() {
    let (enabled, _) = compose(two_spaces(), true);
    let (disabled, text) = compose(two_spaces(), false);

    assert!(disabled.hits.current_space_agents.is_empty());
    assert!(!text.contains(" no agents"));
    // The draggable Spaces/Agents divider only exists in the two-section layout.
    assert_ne!(disabled.hits.sidebar_section_divider, Rect::default());
    assert_eq!(enabled.hits.sidebar_section_divider, Rect::default());
}

#[test]
fn config_defaults_on_and_can_be_disabled() {
    assert!(Config::default().ui.sidebar.current_space.enabled);
    let config: crate::config::SidebarConfig =
        toml::from_str("[current_space]\nenabled = false\n").expect("sidebar config");
    assert!(!config.current_space.enabled);
}

/// `aviv` holds `count` agents, one per tab; `focused_tab` (1-based) holds the focused one.
fn crowded_space(count: usize, focused_tab: usize) -> ClientShellSnapshot {
    let mut projected = two_spaces();
    projected.tabs.retain(|tab| tab.workspace_id != "ws_1");
    projected
        .agents
        .retain(|agent| agent.workspace_id != "ws_1");
    for index in 1..=count {
        let tab_id = format!("crowd_tab_{index}");
        let mut crowd_tab = tab(&tab_id, "ws_1", 100 + index);
        crowd_tab.focused = index == focused_tab;
        projected.tabs.push(crowd_tab);
        let mut crowd_agent = agent(
            &format!("crowd_pane_{index}"),
            "ws_1",
            &tab_id,
            &format!("crowd-{index}"),
        );
        crowd_agent.focused = index == focused_tab;
        projected.agents.push(crowd_agent);
    }
    projected
}

fn first_agents_row(state: &ClientShellState) -> u16 {
    state
        .hits
        .agents
        .iter()
        .map(|(rect, _)| rect.y)
        .min()
        .expect("agent rows")
}

#[test]
fn section_height_does_not_depend_on_agent_count() {
    let (few, _) = compose(crowded_space(1, 1), true);
    let (many, _) = compose(crowded_space(12, 1), true);

    assert_eq!(few.hits.current_space_body, many.hits.current_space_body);
    assert_eq!(first_agents_row(&few), first_agents_row(&many));
}

#[test]
fn overflow_shows_hidden_count_and_scrolls_with_the_wheel() {
    let (mut state, text) = compose(crowded_space(12, 1), true);
    let body = state.hits.current_space_body;
    let shown = state.hits.current_space_agents.len();
    assert!(shown < 12, "fixture should overflow, shown {shown}");
    let more_line = line_at(&text, Rect::new(body.x, body.bottom() - 1, body.width, 1));
    assert_eq!(more_line.trim(), format!("+{} more", 12 - shown));
    assert_eq!(state.hits.current_space_agents[0].1, "crowd_pane_1");

    state.handle_raw_events(vec![RawInputEvent::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: body.x + 2,
        row: body.y,
        modifiers: KeyModifiers::empty(),
    })]);
    state.compose(106, FRAME_ROWS).expect("scrolled frame");
    assert_eq!(state.hits.current_space_agents[0].1, "crowd_pane_2");
}

#[test]
fn focusing_a_hidden_agent_scrolls_it_into_view() {
    let (mut state, _) = compose(crowded_space(12, 1), true);
    assert!(!state
        .hits
        .current_space_agents
        .iter()
        .any(|(_, pane_id)| pane_id == "crowd_pane_9"));

    state.set_snapshot(Box::new(crowded_space(12, 9)));
    state.compose(106, FRAME_ROWS).expect("refocused frame");
    assert!(state
        .hits
        .current_space_agents
        .iter()
        .any(|(_, pane_id)| pane_id == "crowd_pane_9"));
}

#[test]
fn spaces_section_shrinks_to_its_rows() {
    let (state, text) = compose(two_spaces(), true);

    // Two Spaces of two rows each, one blank drop row, then the "new" footer.
    let footer = text
        .lines()
        .position(|line| line.trim_start().starts_with("new"))
        .expect("spaces footer");
    assert_eq!(
        footer,
        state.hits.workspace_body.y as usize + 5,
        "frame: {text}"
    );
    assert_eq!(state.hits.workspace_body.height, 5);
    assert_eq!(state.hits.sidebar_section_divider, Rect::default());
    let heading = state.hits.current_space_body.y - 2;
    assert_eq!(heading as usize, footer + 2, "frame: {text}");
}

#[test]
fn many_spaces_scroll_within_half_the_sidebar() {
    let mut projected = two_spaces();
    for index in 3..=30 {
        let mut workspace = projected.workspaces[1].clone();
        workspace.workspace_id = format!("ws_{index}");
        workspace.number = index;
        workspace.label = format!("space-{index}");
        projected.workspaces.push(workspace);
    }
    let (state, _) = compose(projected, true);

    assert!(state.hits.workspace_max_scroll > 0);
    assert!(state.hits.current_space_body.y <= FRAME_ROWS / 2 + 3);
}
