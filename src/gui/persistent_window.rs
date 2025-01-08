use std::sync::atomic::{AtomicU64, Ordering};

use egui::Context;

/// A struct to give some psuedo-retained state to Egui. Create a `PersistentWindow` and push it to
/// the `PersistentWindowManager` once, then call `render` to continue rendering those
/// `PersistentWindow`s with access to the specified state until they declare they are ready to be removed.
pub struct PersistentWindowManager<S> {
    windows: Vec<PersistentWindow<S>>,
}

static WINDOW_IDS: AtomicU64 = AtomicU64::new(0);

type PersistentWindowFunction<S> =
    Box<dyn FnMut(&u64, &mut Vec<PersistentWindow<S>>, &Context, &mut S) -> bool>;

pub struct PersistentWindow<S> {
    id: u64,
    function: PersistentWindowFunction<S>,
}

impl<S> PersistentWindow<S> {
    pub fn new(function: PersistentWindowFunction<S>) -> PersistentWindow<S> {
        PersistentWindow {
            id: WINDOW_IDS.fetch_add(1, Ordering::Relaxed),
            function,
        }
    }

    /// Egui render function.
    /// Returns false when the window is ready to close, will continue being rendered as long as
    /// true is returned.
    ///
    /// `new_windows` will be added to the list of persistent windows in the manager at the end of
    /// the frame, to allow persistent windows to spawn more persistent windows.
    pub fn render(
        &mut self,
        new_windows: &mut Vec<PersistentWindow<S>>,
        gui_ctx: &Context,
        state: &mut S,
    ) -> bool {
        (self.function)(&self.id, new_windows, gui_ctx, state)
    }
}

impl<S> PersistentWindowManager<S> {
    /// Create a new `PersistentWindowManager`
    pub fn new() -> PersistentWindowManager<S> {
        PersistentWindowManager {
            windows: Vec::new(),
        }
    }

    /// Push a new `PersistentWindow`, which will continue being rendered until it is ready to be
    /// removed.
    pub fn push(&mut self, window: PersistentWindow<S>) {
        self.windows.push(window);
    }

    /// Render all the current `PersistentWindow`s
    pub fn render(&mut self, state: &mut S, gui_ctx: &Context) {
        let mut new_windows: Vec<PersistentWindow<S>> = Vec::new();

        self.windows
            .retain_mut(|window| window.render(&mut new_windows, gui_ctx, state));

        self.windows.append(&mut new_windows);
    }
}
