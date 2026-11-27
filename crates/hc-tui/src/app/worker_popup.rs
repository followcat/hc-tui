//! Workers that have no tab (`ui.nest_worker_terminals`): their agent row
//! is the only way to reach them, and activating it opens the worker in a
//! floating popup over the current tab without moving focus. The popup is
//! an active attach: keys type into the worker, and closing it only
//! detaches — the worker keeps running.
use super::input::TerminalKey;
use super::pane::{Role, Spec};
use super::popup::{PopupState, popup_fit};
use super::*;
use crate::chrome::snapshot::{ClientEndpointId, ClientShellAgent, ClientShellSnapshot};
use crate::popup_size::{PopupSize, resolve_popup_geometry};
use hc_vt::{Action as KeyAction, Key, Mods};

/// What a worker popup knows about the worker beyond its terminal.
pub(super) struct WorkerPopup {
    /// The address of the terminal the worker was started from, for the
    /// title bar.
    pub parent: String,
    /// Under the keyboard lock, the prefix press held back until the next
    /// key says whether it was the chord's first key: anything but `Esc`
    /// sends it to the worker after all, ahead of that key.
    pub held_prefix: Option<TerminalKey>,
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

    /// The pane the open worker popup types into.
    pub(super) fn worker_popup_pane(&mut self) -> Option<&mut super::pane::Pane> {
        self.popup
            .as_mut()
            .filter(|popup| popup.worker.is_some())
            .map(|popup| &mut popup.pane)
    }

    /// The open worker popup's terminal once its pane has a fresh paint: the
    /// worker counts as viewed (acknowledged, `herdr.seen` written).
    pub(super) fn viewed_worker_popup(&self) -> Option<String> {
        let popup = self.popup.as_ref().filter(|p| p.worker.is_some())?;
        popup.pane.has_fresh_paint().then(|| popup.terminal.clone())
    }

    /// The endpoint a terminal address belongs to.
    pub(super) fn endpoint_of(address: &str) -> ClientEndpointId {
        crate::project::machine_of(address).map_or(ClientEndpointId::Local, |m| {
            ClientEndpointId::Remote(m.into())
        })
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

    /// Close a worker popup whose tab lost focus: detach, no kill. Run as
    /// soon as the projection settles rather than at the next watcher
    /// publish, so a tab chord pressed inside the popup neither leaves it
    /// over the new tab nor blocks that tab's float from coming back.
    pub(super) fn close_stale_worker_popup(&mut self) {
        let stale = self.popup.as_ref().is_some_and(|popup| {
            popup.worker.is_some()
                && popup.owner_tab.is_some()
                && popup.owner_tab != self.projection.snapshot.focused_tab_id
        });
        if stale {
            self.close_popup();
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
            worker: Some(WorkerPopup {
                parent,
                held_prefix: None,
            }),
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

    /// The worker popup's two-key chord handling, run before the popup
    /// sees a key. The prefix is held back from the worker, with
    /// `self.prefix` only the armed flag (`Prefix::feed` is never called, so
    /// no hc-tui prefix binding can act on the parent underneath):
    /// prefix then `Esc` closes, then `Enter` keeps, then prefix again
    /// sends one literal prefix to the worker, anything else is dropped.
    /// Under the keyboard lock only the exit survives, so the popup can
    /// always be closed: prefix then `Esc` closes, prefix then prefix sends
    /// one prefix, and prefix then any other key sends the held prefix and
    /// that key to the worker, so nothing typed is lost. Every held-back
    /// press is recorded so its release never reaches the worker; the
    /// key-repeat of a held-back press is ignored. Returns whether the key
    /// was consumed.
    pub(super) fn worker_chord(&mut self, key: &TerminalKey, press: &TerminalKey) -> bool {
        let identity = press.identity();
        if key.input.action != KeyAction::Press {
            return self.popup_key_suppress.contains(&identity) || self.prefix.armed();
        }
        let locked = self.keyboard_locked;
        let is_prefix = self
            .live
            .matches_prefix(&crate::input::terminal_key_of(press));
        if !self.prefix.armed() {
            if is_prefix {
                self.prefix.arm();
                self.popup_key_suppress.insert(identity);
                if let Some(worker) = self.popup.as_mut().and_then(|p| p.worker.as_mut()) {
                    worker.held_prefix = locked.then(|| press.clone());
                }
            }
            return is_prefix;
        }
        self.prefix.reset();
        let held = self
            .popup
            .as_mut()
            .and_then(|popup| popup.worker.as_mut())
            .and_then(|worker| worker.held_prefix.take());
        match press.input.key {
            Key::Escape => {
                self.popup_key_suppress.insert(identity);
                self.close_popup();
            }
            Key::Enter if press.input.mods == Mods::NONE && !locked => {
                self.popup_key_suppress.insert(identity);
                self.keep_worker_popup();
            }
            _ if is_prefix => {
                // A second, distinct press: its release belongs to the worker.
                self.popup_key_suppress.remove(&identity);
                self.popup_key(press);
            }
            _ => match held {
                Some(held) => {
                    self.send_held_prefix(&held);
                    self.popup_key(press);
                }
                None => {
                    self.popup_key_suppress.insert(identity);
                }
            },
        }
        true
    }

    /// Hand a prefix press held back under the lock to the worker after
    /// all. Its release goes through too: the real one if the key is still
    /// down, else a synthetic one right behind the press.
    fn send_held_prefix(&mut self, held: &TerminalKey) {
        self.popup_key(held);
        if !self.popup_key_suppress.remove(&held.identity()) && self.host_reports_key_releases {
            self.popup_key(&held.clone().with_action(KeyAction::Release));
        }
    }

    /// The worker popup's share of key routing, run before the popup sees a
    /// press. A worker popup has its own two-key prefix chord, which under
    /// the lock keeps only its exit, and, unless the keyboard is locked,
    /// yields the four navigation chords (a float yields neither), while
    /// its popup-command chords reach the worker; a locked keyboard types
    /// everything else into it. Returns `None` when the press was consumed,
    /// so the routing function, which returns `None` for a consumed key
    /// too, can `?` it. `None` must never mean "not mine": that is
    /// `Some(())`, which lets the press carry on.
    pub(super) fn worker_popup_key(
        &mut self,
        key: &TerminalKey,
        press: &TerminalKey,
        combo: &crate::config::TerminalKey,
        width: u16,
    ) -> Option<()> {
        use crate::input::{KeybindAction as A, KeybindMatch};
        if self.worker_popup_terminal().is_none() {
            return Some(());
        }
        if self.worker_chord(key, press) {
            return None;
        }
        if self.keyboard_locked {
            return Some(());
        }
        match crate::input::resolve_direct_binding(&self.live.keybinds, combo) {
            Some(KeybindMatch::Action(
                action @ (A::PreviousTab | A::NextTab | A::PreviousAgent | A::NextAgent),
            )) => {
                self.popup_key_suppress.insert(press.identity());
                self.on_action(action, width);
                None
            }
            _ => Some(()),
        }
    }

    /// Send host-committed text to the open worker popup, or else to the
    /// focused pane.
    pub(super) fn send_committed_text(&mut self, text: Vec<u8>) {
        if let Some(pane) = self.worker_popup_pane() {
            pane.input_encoded(text);
        } else {
            self.reset_displayed_scroll();
            self.pane.input_encoded(text);
        }
    }
}
