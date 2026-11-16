use super::TerminalView;
use gpui::{Context, Window};

impl TerminalView {
    pub(crate) fn move_to_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.let_go(cx);
        self.marked = None;
        self.finder = None;
        let refocus =
            |view: &mut Self, window: &mut Window, cx: &mut Context<Self>| view.refocus(window, cx);
        self._focus = [
            cx.observe_window_activation(window, refocus),
            cx.on_focus(&self.focus, window, refocus),
            cx.on_blur(&self.focus, window, refocus),
        ];
        self.refocus(window, cx);
        cx.notify();
    }

    pub fn pid(&self) -> u32 {
        self.pty.pid
    }
}
