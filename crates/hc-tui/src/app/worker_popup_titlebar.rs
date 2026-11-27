//! The worker popup's title bar, drawn over the popup's top border in place
//! of the border title: the worker's status, name and parent, and the
//! `↗ keep` and `×` buttons.
use super::*;
use crate::chrome::extra_hits::PopupButtons;
use crate::chrome::popup_frame::overlay_bg;
use crate::chrome::status::{AgentStatus, status_color, status_icon, status_text};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// The width of the title bar's button strip: `" ↗ keep "` then `" × "`.
const KEEP_WIDTH: u16 = 8;
const CLOSE_WIDTH: u16 = 3;

impl App {
    /// The popup's border title: none for a worker popup, whose title bar
    /// is drawn over the border instead.
    pub(super) fn popup_border_title(&self, title: String) -> String {
        if self.worker_popup_terminal().is_some() {
            String::new()
        } else {
            title
        }
    }

    /// Draw the worker popup's title bar over the top border: the worker's
    /// status dot, name, status and parent on the left, `↗ keep` and `×`
    /// on the right, whose rects become the popup's buttons. `outer` is the
    /// bordered frame, not the gutter around it.
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
        let base = Style::default().bg(overlay_bg(palette));
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
