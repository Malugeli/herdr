# Herdr Sidebar

The vocabulary for herdr's left sidebar and the things it lists, as used in this fork's work on a per-Space agent list.

## Language

### Organisation

**Space**:
A named, top-level container of Tabs; the unit the user switches between (e.g. `aviv`, `thegoal`).
_Avoid_: Workspace (the code's name for it; fine in code, not in conversation or UI text), project, session

**Current Space**:
The Space the client is presenting right now.
_Avoid_: Active workspace, focused workspace

**Tab**:
One of the layouts inside a Space, switched with `prefix+1..9`.
_Avoid_: Window, terminal tab

**Pane**:
One terminal inside a Tab, whether or not an agent runs in it.
_Avoid_: Terminal, window, split

**Agent**:
A Pane in which herdr has detected or been told about a coding agent (Claude, Codex, …). Plain shell Panes are not Agents.
_Avoid_: Terminal, session, bot

### Sidebar sections

**Spaces section**:
The sidebar section listing every Space.

**Agents section**:
The sidebar section listing every Agent across all Spaces.
_Avoid_: Agent panel (the code's name), all-agents list

**Current Space section**:
The sidebar section listing only the Agents of the Current Space; its heading is the Current Space's name.
_Avoid_: Here section, this-space panel, filtered agents
