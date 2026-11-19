//! Bridge a local PCM click to the iOS UI sound service.

#![allow(unsafe_code)]

use std::io::{self, Write};
use std::ptr::NonNull;

use objc2_audio_toolbox::{
    AudioServicesCreateSystemSoundID, AudioServicesDisposeSystemSoundID,
    AudioServicesPlaySystemSound, SystemSoundID,
};
use objc2_core_foundation::CFURL;

use super::waveform;

const SAMPLE_RATE: u32 = 48_000;

pub(super) struct SystemClick {
    id: SystemSoundID,
    _file: tempfile::NamedTempFile,
}

impl SystemClick {
    pub(super) fn open() -> io::Result<Self> {
        let mut file = tempfile::Builder::new()
            .prefix("mytunes-click-")
            .suffix(".wav")
            .tempfile()?;
        write_wave(&mut file)?;
        file.flush()?;
        let url = CFURL::from_file_path(file.path())
            .ok_or_else(|| io::Error::other("Could not create a file URL for the wheel click"))?;
        let mut id = 0;
        // The URL and output pointer are valid for the call. The owned WAV
        // remains available until the returned system sound is disposed.
        let status = unsafe { AudioServicesCreateSystemSoundID(&url, NonNull::from(&mut id)) };
        if status != 0 {
            return Err(io::Error::other(format!("AudioServices status {status}")));
        }
        Ok(Self { id, _file: file })
    }

    pub(super) fn play(&self) {
        // The identifier was allocated by AudioServices and is live for &self.
        // The service's default UI sound property applies the user's settings.
        unsafe { AudioServicesPlaySystemSound(self.id) };
    }
}

impl Drop for SystemClick {
    fn drop(&mut self) {
        // This is the unique owned identifier. The WAV is removed after disposal.
        let _ = unsafe { AudioServicesDisposeSystemSoundID(self.id) };
    }
}

fn write_wave(mut file: impl Write) -> io::Result<()> {
    let samples = waveform(SAMPLE_RATE);
    let bytes = (samples.len() * 2) as u32;
    file.write_all(b"RIFF")?;
    file.write_all(&(bytes + 36).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16_u32.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?; // PCM
    file.write_all(&1_u16.to_le_bytes())?; // Mono
    file.write_all(&SAMPLE_RATE.to_le_bytes())?;
    file.write_all(&(SAMPLE_RATE * 2).to_le_bytes())?;
    file.write_all(&2_u16.to_le_bytes())?;
    file.write_all(&16_u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&bytes.to_le_bytes())?;
    for sample in samples {
        file.write_all(&((sample * f32::from(i16::MAX)).round() as i16).to_le_bytes())?;
    }
    Ok(())
}
