use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "ios")]
mod ios;

const CLICK_MS: u32 = 12;

/// A short feedback sound for the player controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundEffect {
    /// A synthesized wheel detent.
    Click,
}

/// Failure to prepare or request a sound effect.
#[derive(Debug, thiserror::Error)]
pub enum SfxError {
    /// The platform sound service could not prepare the click.
    #[error("The system could not prepare the wheel click")]
    Unavailable,
}

pub(crate) type ClickRequest = Arc<AtomicBool>;

pub(crate) struct Sfx {
    #[cfg(not(target_os = "ios"))]
    request: ClickRequest,
    #[cfg(target_os = "ios")]
    system_click: Option<ios::SystemClick>,
}

impl Sfx {
    pub(crate) fn new() -> Self {
        Self {
            #[cfg(not(target_os = "ios"))]
            request: Arc::new(AtomicBool::new(false)),
            #[cfg(target_os = "ios")]
            system_click: ios::SystemClick::open().ok(),
        }
    }

    pub(crate) fn click_request(&self) -> Option<ClickRequest> {
        #[cfg(not(target_os = "ios"))]
        {
            Some(self.request.clone())
        }
        #[cfg(target_os = "ios")]
        {
            None
        }
    }

    pub(crate) fn play(&self, effect: SoundEffect) -> Result<(), SfxError> {
        match effect {
            SoundEffect::Click => {
                #[cfg(not(target_os = "ios"))]
                self.request.store(true, Ordering::Release);
                #[cfg(target_os = "ios")]
                self.system_click
                    .as_ref()
                    .ok_or(SfxError::Unavailable)?
                    .play();
            }
        }
        Ok(())
    }
}

pub(crate) struct ClickMixer {
    request: Option<ClickRequest>,
    sound: Vec<f32>,
    position: usize,
}

impl ClickMixer {
    pub(crate) fn new(rate: u32, request: Option<ClickRequest>) -> Self {
        let sound = request.as_ref().map_or_else(Vec::new, |_| waveform(rate));
        Self {
            request,
            position: sound.len(),
            sound,
        }
    }

    pub(crate) fn begin_callback(&mut self) {
        if self
            .request
            .as_ref()
            .is_some_and(|request| request.swap(false, Ordering::AcqRel))
        {
            self.position = 0;
        }
    }

    pub(crate) fn active(&self) -> bool {
        self.position < self.sound.len()
    }

    pub(crate) fn next_frame(&mut self) -> f32 {
        if let Some(&sample) = self.sound.get(self.position) {
            self.position += 1;
            sample
        } else {
            0.0
        }
    }
}

fn waveform(rate: u32) -> Vec<f32> {
    let len = (rate * CLICK_MS / 1_000).max(2);
    (0..len)
        .map(|index| {
            let progress = index as f32 / (len - 1) as f32;
            let t = index as f32 / rate as f32;
            let attack = (progress * 18.0).min(1.0);
            let decay = (1.0 - progress).powi(2) * (-5.0 * progress).exp();
            let phase = std::f32::consts::TAU * t;
            0.17 * attack * decay * ((phase * 1_900.0).sin() + 0.4 * (phase * 3_200.0).sin())
        })
        .collect()
}
