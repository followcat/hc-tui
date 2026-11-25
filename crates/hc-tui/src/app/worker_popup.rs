//! Workers that have no tab (`ui.nest_worker_terminals`): their agent row
//! is the only way to reach them, and activating it opens the worker in a
//! floating popup over the current tab without moving focus. The popup is
//! an active attach: keys type into the worker, and closing it only
//! detaches — the worker keeps running.
use super::input::{MouseButton, MouseEventKind, MouseInput};
use super::mouse::contains;
use super::pane::{Role, Spec};
use super::popup::{PopupState, popup_fit};
use super::*;
use crate::chrome::extra_hits::PopupButtons;
use crate::chrome::snapshot::{ClientShellAgent, ClientShellSnapshot};
use crate::chrome::status::{AgentStatus, status_color, status_icon, status_text};
use crate::operations::Action;
use crate::popup_size::{PopupSize, resolve_popup_geometry};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// What a worker popup knows about the worker beyond its terminal.
pub(super) struct WorkerPopup {
    /// The address of the terminal the worker was started from, for the
    /// title bar.
    pub parent: String,
}

/// The width of the title bar's button strip: `" ↗ keep "` then `" × "`.
const KEEP_WIDTH: u16 = 8;
const CLOSE_WIDTH: u16 = 3;

/// Whether `popup` is a worker popup whose worker has finished — the
/// reconcile arm that closes it.
pub(super) fn finished(view: &View, popup: &PopupState) -> bool {
    popup.worker.is_some()
        && view
            .inventory
            .get(&popup.terminal)
            .is_some_and(|term| term.state == "finished")
}

impl App {
    /// The agent record of `pane_id` on any endpoint.
    fn agent_record_of(&self, pane_id: &str) -> Option<&ClientShellAgent> {
        fn find<'a>(snapshot: &'a ClientShellSnapshot, id: &str) -> Option<&'a ClientShellAgent> {
            snapshot.agents.iter().find(|agent| agent.pane_id == id)
        }
        find(&self.projection.snapshot, pane_id).or_else(|| {
            self.projection
                .endpoints
                .iter()
                .filter_map(|endpoint| endpoint.snapshot.as_deref())
                .find_map(|snapshot| find(snapshot, pane_id))
        })
    }

    /// Whether `pane_id` is the row of a worker with no tab and no pane,
    /// on any endpoint. Every place that activates an agent asks this
    /// first: such an id must never become `self.focused`, because no pane
    /// exists to focus.
    pub(super) fn worker_popup_agent(&self, pane_id: &str) -> bool {
        self.agent_record_of(pane_id)
            .is_some_and(|agent| agent.worker_popup)
    }

    /// The terminal of the open worker popup, if one is open.
    pub(super) fn worker_popup_terminal(&self) -> Option<&str> {
        self.popup
            .as_ref()
            .filter(|popup| popup.worker.is_some())
            .map(|popup| popup.terminal.as_str())
    }

    /// The open worker popup's terminal once its pane has a fresh paint: the
    /// worker counts as viewed (acknowledged, `herdr.seen` written).
    pub(super) fn viewed_worker_popup(&self) -> Option<String> {
        let popup = self.popup.as_ref().filter(|p| p.worker.is_some())?;
        popup.pane.has_fresh_paint().then(|| popup.terminal.clone())
    }

    /// A click on an agent row: open a worker's row in place, focus any
    /// other agent's pane (closing an open worker popup first, so it never
    /// stays over a pane navigation moved to).
    pub(super) fn activate_agent(&mut self, pane_id: &str) {
        if self.worker_popup_agent(pane_id) {
            self.open_worker(pane_id.to_owned());
        } else {
            self.close_worker_popup();
            self.focus_pane(pane_id.to_owned());
        }
    }

    /// Close the open worker popup, if any: detach, no kill.
    pub(super) fn close_worker_popup(&mut self) {
        if self.worker_popup_terminal().is_some() {
            self.close_popup();
        }
    }

    /// Open the worker `pane_id` for an activated row.
    pub(super) fn open_worker(&mut self, pane_id: String) {
        self.open_worker_popup(pane_id);
    }

    /// Open `terminal` in the interactive worker popup, replacing whatever
    /// popup is open: a persistent float parks, a command popup closes.
    pub(super) fn open_worker_popup(&mut self, terminal: String) {
        let Some(term) = self
            .view
            .inventory
            .get(&terminal)
            .filter(|term| term.state != "finished")
        else {
            self.show_notice("Worker unavailable", "the worker is not running");
            return;
        };
        let (created_ms, parent) = (term.created_ms, term.parent.clone().unwrap_or_default());
        self.dismiss_popup();
        self.close_peek();
        self.popup_selection = None;
        self.popup_gesture = None;
        let mut pane = self.new_pane();
        pane.want(Some(Spec {
            terminal: terminal.clone(),
            role: Role::Active,
        }));
        pane.on_view(&self.view);
        let (width, height) = (Some(PopupSize::Percent(80)), Some(PopupSize::Percent(80)));
        if let Some(geometry) = resolve_popup_geometry(width, height, self.frame_area()) {
            popup_fit(&mut pane, geometry);
        }
        self.popup = Some(PopupState {
            pane,
            terminal,
            width,
            height,
            owner_tab: self.projection.snapshot.focused_tab_id.clone(),
            persistent: false,
            key: None,
            created_ms,
            seen: true,
            worker: Some(WorkerPopup { parent }),
        });
        self.sched.mark_dirty(Instant::now());
    }

    /// Adopt `terminal` into a fresh tab of the focused space — the layout
    /// CAS plus `terminal.keep`. Refused for a terminal of another machine
    /// than the focused one: the labels creation writes name this
    /// machine's space and tab.
    pub(super) fn keep_terminal(
        &mut self,
        terminal: String,
        created_ms: u64,
    ) -> Result<(), String> {
        let focused = self.focused.as_deref().and_then(crate::project::machine_of);
        if crate::project::machine_of(&terminal) != focused {
            return Err("Keep it from a tab on the worker's machine".into());
        }
        let creation = self.creation(None, false)?;
        let labels = creation
            .params
            .labels
            .iter()
            .filter(|(k, _)| k.as_str() != "herdr.seen")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        self.submit_action(Action::PeekKeep {
            edit: creation.edit,
            terminal,
            created_ms,
            labels,
            order: creation.order,
        });
        Ok(())
    }

    /// `↗ keep`: adopt the worker into a fresh tab of the focused space and
    /// close the popup (detach only). A refusal leaves the popup open.
    pub(super) fn keep_worker_popup(&mut self) {
        let Some(popup) = self.popup.as_ref().filter(|p| p.worker.is_some()) else {
            return;
        };
        let created_ms = self
            .view
            .inventory
            .get(&popup.terminal)
            .map_or(popup.created_ms, |term| term.created_ms);
        match self.keep_terminal(popup.terminal.clone(), created_ms) {
            Ok(()) => self.close_popup(),
            Err(error) => self.show_notice("Keep unavailable", &error),
        }
    }

    /// Whether `point` is on an agent row of the last frame.
    pub(super) fn agent_row_at(&self, point: (u16, u16)) -> bool {
        self.hit_map
            .agents
            .iter()
            .any(|(rect, _)| contains(*rect, point))
            || self
                .hit_map
                .endpoint_agents
                .iter()
                .any(|(rect, _, _)| contains(*rect, point))
    }

    /// A left press outside the open popup that lands on an agent row
    /// dismisses the popup and returns `true` so the row handles the same
    /// press: a float parks, a command popup closes, a worker popup
    /// detaches. Any other outside press keeps its own handling.
    pub(super) fn click_through_popup(&mut self, mouse: &MouseInput) -> bool {
        let point = (mouse.col, mouse.row);
        let outside = self
            .hit_map
            .popup
            .as_ref()
            .is_some_and(|hit| !contains(hit.rect, point));
        let through = mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && outside
            && self.agent_row_at(point);
        if through {
            self.dismiss_popup();
        }
        through
    }

    /// The worker popup's title-bar buttons and outside press. `None` when
    /// the press is not the popup chrome's to take (inside the surface,
    /// another button or another popup kind), so today's forwarding runs.
    pub(super) fn worker_popup_press(&mut self, mouse: &MouseInput) -> Option<bool> {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return None;
        }
        self.worker_popup_terminal()?;
        let point = (mouse.col, mouse.row);
        if let Some(buttons) = self.hit_map.extra.popup_buttons {
            if contains(buttons.keep, point) {
                self.keep_worker_popup();
                return Some(true);
            }
            if contains(buttons.close, point) {
                self.close_popup();
                return Some(true);
            }
        }
        let outer = self.hit_map.popup.as_ref()?.rect;
        if contains(outer, point) {
            return None;
        }
        self.close_popup();
        Some(true)
    }

    /// Draw the worker popup's title bar over the top border: the worker's
    /// status dot, name, status and parent on the left, `↗ keep` and `×`
    /// on the right, whose rects become the popup's buttons.
    pub(super) fn draw_worker_titlebar(&mut self, buf: &mut ratatui::buffer::Buffer, outer: Rect) {
        let Some(popup) = self.popup.as_ref().filter(|p| p.worker.is_some()) else {
            return;
        };
        let parent = popup.worker.as_ref().map(|w| w.parent.clone());
        let terminal = popup.terminal.clone();
        let agent = self.agent_record_of(&terminal);
        let status = agent
            .map(|agent| agent.agent_status)
            .filter(|status| *status != AgentStatus::Unknown);
        let name = agent
            .and_then(|agent| agent.name.clone())
            .or_else(|| {
                self.view
                    .inventory
                    .get(&terminal)
                    .and_then(|t| t.name.clone())
            })
            .unwrap_or_else(|| terminal.clone());
        let palette = &self.shell.palette;
        let base = Style::default().bg(palette.panel_bg);
        let dim = base.fg(palette.overlay0);
        let mut spans = vec![Span::styled(" ", base)];
        if let Some(status) = status {
            let style = base.fg(status_color(status, palette));
            spans.push(Span::styled(
                status_icon(status, self.shell.status_indicators),
                style,
            ));
            spans.push(Span::styled(" ", base));
        }
        spans.push(Span::styled(
            name,
            base.fg(palette.text).add_modifier(Modifier::BOLD),
        ));
        if let Some(status) = status {
            spans.push(Span::styled(
                format!("  {}", status_text(status)),
                base.fg(status_color(status, palette)),
            ));
        }
        if let Some(parent) = parent.filter(|parent| !parent.is_empty()) {
            spans.push(Span::styled(format!("  from {parent}"), dim));
        }
        spans.push(Span::styled(" ", base));
        let strip = KEEP_WIDTH + CLOSE_WIDTH;
        let keep_x = outer.right().saturating_sub(1 + strip).max(outer.x + 1);
        let room = keep_x.saturating_sub(outer.x + 1);
        buf.set_line(outer.x + 1, outer.y, &Line::from(spans), room);
        let accent = base.fg(palette.accent);
        buf.set_line(
            keep_x,
            outer.y,
            &Line::from(vec![
                Span::styled(" ↗ keep ", accent),
                Span::styled(" × ", accent),
            ]),
            strip,
        );
        self.hit_map.extra.popup_buttons = Some(PopupButtons {
            keep: Rect::new(keep_x, outer.y, KEEP_WIDTH, 1),
            close: Rect::new(keep_x + KEEP_WIDTH, outer.y, CLOSE_WIDTH, 1),
        });
    }
}
