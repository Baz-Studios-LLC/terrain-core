//! Taking work back.
//!
//! Both painted layers want the same thing: a drag lasting two hundred frames
//! comes back in ONE press, not two hundred, and a fresh edit after an undo ends
//! the branch the way every editor does. The ground wants it and the woods want
//! it, and they are the same want — a grid of numbers, remembering what a cell
//! held before a stroke first touched it.
//!
//! So it is written once here rather than twice out there. What differs between
//! the two — what a cell MEANS, where it sits, how far a change reaches — stays
//! with each of them; this only remembers.

use std::collections::HashMap;

/// The cells one stroke changed, and what they held before it.
pub(crate) type Stroke = HashMap<usize, f32>;

pub(crate) struct History {
    /// Cells touched since the current stroke opened, and their prior values.
    /// `None` between strokes.
    recording: Option<Stroke>,
    undone: Vec<Stroke>,
    redone: Vec<Stroke>,
    /// How many strokes can be taken back. Each holds only the cells it touched,
    /// so memory is bounded by ground worked rather than by the size of the
    /// world.
    depth: usize,
}

impl History {
    pub(crate) fn new(depth: usize) -> Self {
        Self {
            recording: None,
            undone: Vec::new(),
            redone: Vec::new(),
            depth,
        }
    }

    /// Opens a group. Everything recorded until [`Self::end`] comes back at once.
    pub(crate) fn begin(&mut self) {
        self.recording = Some(HashMap::new());
        // A fresh edit ends the redo branch, the same as every editor.
        self.redone.clear();
    }

    pub(crate) fn end(&mut self) {
        let Some(stroke) = self.recording.take() else {
            return;
        };
        // A stroke that touched nothing is not a press worth spending.
        if stroke.is_empty() {
            return;
        }
        self.undone.push(stroke);
        if self.undone.len() > self.depth {
            self.undone.remove(0);
        }
    }

    /// Remembers what a cell held, if a stroke is open.
    ///
    /// Only what it held before the STROKE began — hence `or_insert` — so
    /// replaying a long drag backwards lands on the right ground rather than on
    /// the frame before last.
    pub(crate) fn record(&mut self, cell: usize, was: f32) {
        if let Some(recording) = &mut self.recording {
            recording.entry(cell).or_insert(was);
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undone.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redone.is_empty()
    }

    // The caller writes the values back itself and hands over what was there, so
    // that this stays ignorant of what a cell is and of how a layer counts them.

    pub(crate) fn take_undo(&mut self) -> Option<Stroke> {
        self.undone.pop()
    }

    pub(crate) fn push_redo(&mut self, inverse: Stroke) {
        self.redone.push(inverse);
    }

    pub(crate) fn take_redo(&mut self) -> Option<Stroke> {
        self.redone.pop()
    }

    pub(crate) fn push_undo(&mut self, inverse: Stroke) {
        self.undone.push(inverse);
    }
}
