//! Mouse handling around the popups: a press outside one that lands on an
//! agent row or space header goes through to it, and the worker popup's own
//! title-bar buttons and outside press.
use super::input::{MouseButton, MouseEventKind, MouseInput};
use super::mouse::contains;
use super::*;

impl App {
    /// Whether `point` is on an agent row or an agent space header of the
    /// last frame.
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
            || self
                .hit_map
                .extra
                .agent_space_headers
                .iter()
                .any(|(rect, _)| contains(*rect, point))
            || self
                .hit_map
                .extra
                .endpoint_agent_space_headers
                .iter()
                .any(|(rect, _, _)| contains(*rect, point))
    }

    /// With `nest_worker_terminals` on, whether a press lands on an agent row
    /// or space header, which then acts on it through an open popup or peek.
    pub(super) fn press_goes_through(&self, mouse: &MouseInput) -> bool {
        self.shell.nest_worker_terminals && self.agent_row_at((mouse.col, mouse.row))
    }

    /// A press while a popup is open, as far as workers go: a click through
    /// to an agent row (`Some(false)`: the row takes the press), else the
    /// worker popup's own chrome (`worker_popup_press`). `None` leaves the
    /// press to the popup's own handling.
    pub(super) fn worker_popup_mouse(&mut self, mouse: &MouseInput) -> Option<bool> {
        if self.click_through_popup(mouse) {
            return Some(false);
        }
        self.worker_popup_press(mouse)
    }

    /// With `nest_worker_terminals` on, a left press outside the open popup
    /// that lands on an agent row or space header dismisses the popup and
    /// returns `true` so the row handles the same press: a float parks, a
    /// command popup closes, a worker popup detaches. Any other outside
    /// press, and every press with the setting off, keeps its own handling.
    pub(super) fn click_through_popup(&mut self, mouse: &MouseInput) -> bool {
        let point = (mouse.col, mouse.row);
        let outside = self
            .hit_map
            .popup
            .as_ref()
            .is_some_and(|hit| !contains(hit.rect, point));
        let through = mouse.kind == MouseEventKind::Down(MouseButton::Left)
            && outside
            && self.press_goes_through(mouse);
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
}
