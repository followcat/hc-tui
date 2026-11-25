//! Workers that have no tab (`ui.nest_worker_terminals`): their agent row
//! is the only way to reach them, and activating it opens the worker in a
//! floating popup over the current tab without moving focus. The popup is
//! an active attach: keys type into the worker, and closing it only
//! detaches — the worker keeps running.
use super::pane::{Role, Spec};
use super::popup::{PopupState, popup_fit};
use super::*;
use crate::chrome::snapshot::{ClientShellAgent, ClientShellSnapshot};
use crate::popup_size::{PopupSize, resolve_popup_geometry};

/// What a worker popup knows about the worker beyond its terminal.
pub(super) struct WorkerPopup {
    /// The address of the terminal the worker was started from, for the
    /// title bar.
    pub parent: String,
}

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
    pub(super) fn agent_record_of(&self, pane_id: &str) -> Option<&ClientShellAgent> {
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

    /// The rest of closing a worker popup once its attach is dropped: the
    /// worker is not the popup's to kill, and an armed prefix of its chord
    /// handling never outlives it. `false` for any other popup, which the
    /// caller then kills.
    pub(super) fn finish_worker_popup_close(&mut self, popup: &PopupState) -> bool {
        if popup.worker.is_none() {
            return false;
        }
        self.prefix.reset();
        self.sched.mark_dirty(Instant::now());
        true
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
        // A prefix armed in the pane underneath is not the worker's: its
        // first key must not be taken as that chord's second key.
        self.prefix.reset();
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
}
