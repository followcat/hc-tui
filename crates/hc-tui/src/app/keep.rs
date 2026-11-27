//! Adopting a terminal into a tab of the focused space.
use super::*;
use crate::operations::Action;

impl App {
    /// Adopt `terminal` into a fresh tab of the focused space: the layout
    /// CAS plus `terminal.keep`. Refused for a terminal of another machine
    /// than the focused one, since the labels creation writes name the
    /// focused machine's space and tab.
    pub(super) fn keep_terminal(
        &mut self,
        terminal: String,
        created_ms: u64,
    ) -> Result<(), String> {
        let focused = self.focused.as_deref().and_then(crate::project::machine_of);
        if crate::project::machine_of(&terminal) != focused {
            return Err("Keep it from a tab on that terminal's machine".into());
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
}
