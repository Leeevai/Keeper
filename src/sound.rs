//! Typing sounds, ported from `@keybr/textinput-sounds`.

use crate::settings::{PlaySounds, SoundTheme};
use crate::textinput::Feedback;

/// What to play for a key press, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    Click,
    Blip,
}

/// Decides which sound a key press makes, the same way as keybr.com.
pub fn cue(mode: PlaySounds, feedback: Feedback) -> Option<Cue> {
    let typed = matches!(feedback, Feedback::Succeeded | Feedback::Recovered);
    match mode {
        PlaySounds::None => None,
        PlaySounds::All => Some(if typed { Cue::Click } else { Cue::Blip }),
        PlaySounds::ErrorsOnly => (!typed).then_some(Cue::Blip),
        PlaySounds::KeysOnly => typed.then_some(Cue::Click),
    }
}

/// The recorded samples of a sound theme.
pub fn samples(theme: SoundTheme) -> Vec<&'static [u8]> {
    macro_rules! files {
        ($($name:literal),*) => { vec![$(include_bytes!(concat!("../assets/sounds/", $name, ".mp3")).as_slice()),*] };
    }
    match theme {
        SoundTheme::Default => files!("default-click"),
        SoundTheme::Mechanical1 => files!(
            "mechanical1-click1",
            "mechanical1-click2",
            "mechanical1-click3",
            "mechanical1-click4",
            "mechanical1-click5",
            "mechanical1-click6",
            "mechanical1-click7",
            "mechanical1-click8",
            "mechanical1-click9"
        ),
        SoundTheme::Mechanical2 => files!(
            "mechanical2-click1",
            "mechanical2-click2",
            "mechanical2-click3",
            "mechanical2-click4",
            "mechanical2-click5"
        ),
        SoundTheme::Typewriter1 => files!(
            "typewriter1-click1",
            "typewriter1-click2",
            "typewriter1-click3",
            "typewriter1-click4",
            "typewriter1-click5",
            "typewriter1-click6",
            "typewriter1-click7",
            "typewriter1-click8",
            "typewriter1-click9"
        ),
        SoundTheme::Typewriter2 => files!(
            "typewriter2-click1",
            "typewriter2-click2",
            "typewriter2-click3",
            "typewriter2-click4",
            "typewriter2-click5"
        ),
    }
}

pub const BLIP: &[u8] = include_bytes!("../assets/sounds/blip.mp3");

/// Plays typing sounds on the default audio output. The output is opened
/// on the first sound, so nothing happens while sounds are off.
#[derive(Default)]
pub struct Sounds {
    #[cfg(feature = "sound")]
    player: Option<player::Player>,
    /// Opening the audio output failed; do not try again.
    failed: bool,
    next: usize,
}

impl Sounds {
    pub fn play(&mut self, mode: PlaySounds, theme: SoundTheme, volume: f64, feedback: Feedback) {
        let Some(cue) = cue(mode, feedback) else {
            return;
        };
        if self.failed || volume <= 0.0 {
            return;
        }
        self.next = self.next.wrapping_add(7);
        #[cfg(feature = "sound")]
        {
            if self.player.is_none() {
                match player::Player::open() {
                    Some(p) => self.player = Some(p),
                    None => {
                        self.failed = true;
                        return;
                    }
                }
            }
            if let Some(player) = &mut self.player {
                player.play(cue, theme, volume as f32, self.next);
            }
        }
        #[cfg(not(feature = "sound"))]
        {
            let _ = (cue, theme);
            self.failed = true;
        }
    }
}

#[cfg(feature = "sound")]
mod player {
    use std::collections::HashMap;
    use std::io::Cursor;

    use rodio::buffer::SamplesBuffer;
    use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source};

    use super::{BLIP, Cue, samples};
    use crate::settings::SoundTheme;

    pub struct Player {
        sink: MixerDeviceSink,
        clicks: HashMap<SoundTheme, Vec<SamplesBuffer>>,
        blip: Option<SamplesBuffer>,
    }

    /// Decodes a recording once, so that playing it costs nothing.
    pub fn decode(data: &'static [u8]) -> Option<SamplesBuffer> {
        let decoder = Decoder::new(Cursor::new(data)).ok()?;
        let channels = decoder.channels();
        let rate = decoder.sample_rate();
        let samples: Vec<rodio::Sample> = decoder.collect();
        Some(SamplesBuffer::new(channels, rate, samples))
    }

    impl Player {
        pub fn open() -> Option<Self> {
            let mut sink = DeviceSinkBuilder::open_default_sink().ok()?;
            sink.log_on_drop(false);
            Some(Self {
                sink,
                clicks: HashMap::new(),
                blip: None,
            })
        }

        pub fn play(&mut self, cue: Cue, theme: SoundTheme, volume: f32, pick: usize) {
            let buffer = match cue {
                Cue::Blip => {
                    if self.blip.is_none() {
                        self.blip = decode(BLIP);
                    }
                    self.blip.clone()
                }
                Cue::Click => {
                    let clicks = self
                        .clicks
                        .entry(theme)
                        .or_insert_with(|| samples(theme).into_iter().filter_map(decode).collect());
                    (!clicks.is_empty()).then(|| clicks[pick % clicks.len()].clone())
                }
            };
            if let Some(buffer) = buffer {
                self.sink.mixer().add(buffer.amplify(volume));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cues_follow_keybr() {
        use Feedback::*;
        assert_eq!(cue(PlaySounds::None, Failed), None);
        assert_eq!(cue(PlaySounds::All, Succeeded), Some(Cue::Click));
        assert_eq!(cue(PlaySounds::All, Recovered), Some(Cue::Click));
        assert_eq!(cue(PlaySounds::All, Failed), Some(Cue::Blip));
        assert_eq!(cue(PlaySounds::ErrorsOnly, Succeeded), None);
        assert_eq!(cue(PlaySounds::ErrorsOnly, Failed), Some(Cue::Blip));
        assert_eq!(cue(PlaySounds::KeysOnly, Succeeded), Some(Cue::Click));
        assert_eq!(cue(PlaySounds::KeysOnly, Failed), None);
    }

    #[test]
    fn every_theme_has_samples() {
        for theme in SoundTheme::ALL {
            let files = samples(theme);
            assert!(!files.is_empty());
            assert!(files.iter().all(|f| f.len() > 100));
        }
    }

    #[cfg(feature = "sound")]
    #[test]
    fn samples_decode() {
        use rodio::Source;
        for theme in SoundTheme::ALL {
            for data in samples(theme) {
                let buffer = player::decode(data).expect("a valid mp3");
                assert!(buffer.total_duration().unwrap().as_millis() > 10);
            }
        }
        assert!(player::decode(BLIP).is_some());
        assert!(player::decode(b"not an mp3").is_none());
    }

    /// Plays a few sounds on the real audio output: `cargo test -- --ignored`.
    #[cfg(feature = "sound")]
    #[test]
    #[ignore]
    fn plays_on_the_audio_output() {
        let mut sounds = Sounds::default();
        for theme in SoundTheme::ALL {
            sounds.play(PlaySounds::All, theme, 0.3, Feedback::Succeeded);
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        sounds.play(PlaySounds::All, SoundTheme::Default, 0.3, Feedback::Failed);
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(!sounds.failed, "no audio output");
    }
}
