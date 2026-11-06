//! In-memory device used by `--dry-run` and unit tests.

use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::color::Rgb;
use crate::device::Device;
use crate::geometry::LedSpec;

#[derive(Clone, Debug, Default)]
pub struct MockState {
    pub frames: Vec<Vec<Rgb>>,
    pub restore_calls: usize,
    pub restored_colors: Option<Vec<Rgb>>,
}

pub struct MockDevice {
    id: String,
    kind: &'static str,
    layout: Vec<LedSpec>,
    opened: bool,
    state: Arc<Mutex<MockState>>,
    restore_color: Rgb,
    max_recorded_frames: usize,
}

impl MockDevice {
    pub fn new(
        id: impl Into<String>,
        kind: &'static str,
        layout: Vec<LedSpec>,
        restore_color: Rgb,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            layout,
            opened: false,
            state: Arc::new(Mutex::new(MockState::default())),
            restore_color,
            max_recorded_frames: 600,
        }
    }

    #[cfg(test)]
    pub fn state(&self) -> Arc<Mutex<MockState>> {
        Arc::clone(&self.state)
    }
}

impl Device for MockDevice {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> &'static str {
        self.kind
    }

    fn model(&self) -> &'static str {
        "mock device"
    }

    fn layout(&self) -> &[LedSpec] {
        &self.layout
    }

    fn open(&mut self) -> Result<()> {
        self.opened = true;
        Ok(())
    }

    fn send(&mut self, colors: &[Rgb]) -> Result<()> {
        if !self.opened {
            anyhow::bail!("mock {} is not open", self.id);
        }
        if colors.len() != self.layout.len() {
            anyhow::bail!(
                "mock {} expected {} colors, got {}",
                self.id,
                self.layout.len(),
                colors.len()
            );
        }
        let mut state = self.state.lock().expect("mock state poisoned");
        if state.frames.len() < self.max_recorded_frames {
            state.frames.push(colors.to_vec());
        }
        Ok(())
    }

    fn restore(&mut self) -> Result<()> {
        let mut state = self.state.lock().expect("mock state poisoned");
        state.restore_calls += 1;
        state.restored_colors = Some(vec![self.restore_color; self.layout.len()]);
        Ok(())
    }

    fn close(&mut self) {
        self.opened = false;
    }

    fn is_open(&self) -> bool {
        self.opened
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_frames_and_restore() {
        let layout = vec![LedSpec::new("x", 0.5, 0.5)];
        let mut device = MockDevice::new("m", "mock", layout, Rgb::BLACK);
        device.open().unwrap();
        device.send(&[Rgb::WHITE]).unwrap();
        device.restore().unwrap();
        let state = device.state();
        let state = state.lock().unwrap();
        assert_eq!(state.frames, vec![vec![Rgb::WHITE]]);
        assert_eq!(state.restore_calls, 1);
        assert_eq!(state.restored_colors, Some(vec![Rgb::BLACK]));
    }
}
