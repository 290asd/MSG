//! A short, quiet tick when the slide changes (`slideTick`). The sound is made here, so there is no file for it.

use std::sync::OnceLock;

const RATE: u32 = 44_100;

/// A WAV of about 25 ms: a 2 kHz tone that dies away fast.
// ponytail: fixed sound and volume; a setting if it is too loud or too quiet.
fn wav() -> Vec<u8> {
    let samples: Vec<i16> = (0..RATE / 40)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            ((t * 2000.0 * std::f32::consts::TAU).sin() * (-t * 250.0).exp() * 6000.0) as i16
        })
        .collect();
    let data_len = samples.len() as u32 * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[link(name = "winmm")]
extern "system" {
    fn PlaySoundW(sound: *const u8, module: *mut std::ffi::c_void, flags: u32) -> i32;
}

const SND_ASYNC: u32 = 0x1;
const SND_NODEFAULT: u32 = 0x2;
const SND_MEMORY: u32 = 0x4;

/// Plays the tick without waiting for it; a new one cuts the last one short.
pub fn play() {
    // SND_MEMORY reads the buffer while the sound plays, so it lives as long as the program.
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    let wav = WAV.get_or_init(wav);
    // SAFETY: the buffer is a whole WAV in memory and is never freed.
    unsafe {
        PlaySoundW(wav.as_ptr(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn wav_header_matches_data() {
        let wav = super::wav();
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()) as usize, wav.len() - 8);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize, wav.len() - 44);
    }
}
