//! Bulk changes to the sounds of a table: downmixing playfield sounds to
//! mono.
//!
//! vpinball positions a playfield sound in the cabinet speakers from the
//! item that plays it, which needs a single channel; a stereo sound is
//! averaged to mono when it is decoded and its table audit reports it as
//! an error. Most tables carry such sounds anyway: in a corpus of 1361
//! tables, 1098 had them, 30 thousand sounds and 9 GB of samples. Storing
//! them mono halves that, with the same result as vpinball's own mix.
//!
//! Backglass sounds are meant to be stereo and are left alone.

use super::VPX;
use super::sound::{OutputTarget, SoundData};
use std::fmt;
use std::io;

/// PCM integer samples
const WAVE_FORMAT_PCM: u16 = 1;
/// IEEE float samples
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;

/// What [`VPX::playfield_sounds_to_mono`] did with one sound
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SoundToMono {
    /// The sound was downmixed
    Converted {
        name: String,
        channels: u16,
        bytes_before: usize,
        bytes_after: usize,
    },
    /// The sound is stereo but was left as it was
    Skipped {
        name: String,
        reason: MonoSkipReason,
    },
}

/// Why [`VPX::playfield_sounds_to_mono`] left a stereo sound alone
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MonoSkipReason {
    /// The sound is not a wav; compressed formats are not decoded
    NotWav(String),
    /// The wav has a sample format this library does not mix
    UnsupportedFormat { format_tag: u16, bits: u16 },
}

impl fmt::Display for SoundToMono {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoundToMono::Converted {
                name,
                channels,
                bytes_before,
                bytes_after,
            } => write!(
                f,
                "sound {name:?}: {channels} channels -> mono, {bytes_before} -> {bytes_after} bytes"
            ),
            SoundToMono::Skipped { name, reason } => write!(f, "sound {name:?} skipped: {reason}"),
        }
    }
}

impl fmt::Display for MonoSkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonoSkipReason::NotWav(ext) => write!(f, "{ext} sounds are not decoded"),
            MonoSkipReason::UnsupportedFormat { format_tag, bits } => {
                write!(
                    f,
                    "wav format {format_tag} with {bits} bits is not supported"
                )
            }
        }
    }
}

impl VPX {
    /// Downmixes every stereo (or wider) sound that plays on the playfield
    /// speakers to mono, the way vpinball mixes it at playback. Backglass
    /// sounds keep their channels. Returns what happened to every sound
    /// that had more than one channel.
    pub fn playfield_sounds_to_mono(&mut self) -> Vec<SoundToMono> {
        let mut results = Vec::new();
        for sound in &mut self.sounds {
            if sound.output_target != OutputTarget::Table || sound.wave_form.channels <= 1 {
                continue;
            }
            let name = sound.name.clone();
            let channels = sound.wave_form.channels;
            let bytes_before = sound.data.len();
            results.push(match sound.to_mono() {
                Ok(true) => SoundToMono::Converted {
                    name,
                    channels,
                    bytes_before,
                    bytes_after: sound.data.len(),
                },
                Ok(false) => SoundToMono::Skipped {
                    name,
                    reason: MonoSkipReason::NotWav(sound_extension(sound)),
                },
                Err(_) => SoundToMono::Skipped {
                    name,
                    reason: MonoSkipReason::UnsupportedFormat {
                        format_tag: sound.wave_form.format_tag,
                        bits: sound.wave_form.bits_per_sample,
                    },
                },
            });
        }
        results
    }
}

/// The lower case extension of the sound's import path; no extension
/// means wav, as in vpinball
fn sound_extension(sound: &SoundData) -> String {
    sound
        .path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_lowercase())
        .unwrap_or_else(|| "wav".to_string())
}

impl SoundData {
    /// Averages the channels of a wav sound into one, which is what
    /// vpinball does when it plays a stereo sound on the playfield.
    /// Returns `false` when the sound is mono already or not a wav.
    ///
    /// # Errors
    ///
    /// [`io::ErrorKind::Unsupported`] for a sample format other than 8,
    /// 16, 24 or 32 bit PCM or 32 bit float.
    pub fn to_mono(&mut self) -> io::Result<bool> {
        let channels = usize::from(self.wave_form.channels);
        if channels <= 1 || sound_extension(self) != "wav" {
            return Ok(false);
        }
        let bits = self.wave_form.bits_per_sample;
        let bytes_per_sample = usize::from(bits / 8);
        let mono = match (self.wave_form.format_tag, bits) {
            (WAVE_FORMAT_PCM, 8) => mix(
                &self.data,
                channels,
                1,
                |sample| {
                    // 8 bit samples are unsigned around 128
                    i64::from(sample[0]) - 128
                },
                |value, out| out.push((value + 128) as u8),
            ),
            (WAVE_FORMAT_PCM, 16) => mix(
                &self.data,
                channels,
                2,
                |sample| i64::from(i16::from_le_bytes([sample[0], sample[1]])),
                |value, out| out.extend_from_slice(&(value as i16).to_le_bytes()),
            ),
            (WAVE_FORMAT_PCM, 24) => mix(
                &self.data,
                channels,
                3,
                |sample| i64::from(i32::from_le_bytes([0, sample[0], sample[1], sample[2]]) >> 8),
                |value, out| out.extend_from_slice(&(value as i32).to_le_bytes()[..3]),
            ),
            (WAVE_FORMAT_PCM, 32) => mix(
                &self.data,
                channels,
                4,
                |sample| {
                    i64::from(i32::from_le_bytes([
                        sample[0], sample[1], sample[2], sample[3],
                    ]))
                },
                |value, out| out.extend_from_slice(&(value as i32).to_le_bytes()),
            ),
            (WAVE_FORMAT_IEEE_FLOAT, 32) => mix_float(&self.data, channels),
            (format_tag, bits) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("wav format {format_tag} with {bits} bits"),
                ));
            }
        };
        self.data = mono;
        self.wave_form.channels = 1;
        self.wave_form.block_align = bytes_per_sample as u16;
        self.wave_form.avg_bytes_per_sec = self.wave_form.samples_per_sec * bytes_per_sample as u32;
        Ok(true)
    }
}

/// Averages the integer samples of every frame; a trailing partial frame
/// is dropped
fn mix(
    data: &[u8],
    channels: usize,
    bytes_per_sample: usize,
    decode: impl Fn(&[u8]) -> i64,
    encode: impl Fn(i64, &mut Vec<u8>),
) -> Vec<u8> {
    let frame = channels * bytes_per_sample;
    let mut out = Vec::with_capacity(data.len() / channels);
    for frame in data.chunks_exact(frame) {
        let sum: i64 = frame.chunks_exact(bytes_per_sample).map(&decode).sum();
        // rounded average, away from zero at .5
        let channels = channels as i64;
        let average = (sum + sum.signum() * channels / 2) / channels;
        encode(average, &mut out);
    }
    out
}

fn mix_float(data: &[u8], channels: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / channels);
    for frame in data.chunks_exact(channels * 4) {
        let sum: f32 = frame
            .as_chunks::<4>()
            .0
            .iter()
            .map(|sample| f32::from_le_bytes(*sample))
            .sum();
        out.extend_from_slice(&(sum / channels as f32).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx;
    use crate::vpx::sound::WaveForm;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    fn sound(
        name: &str,
        ext: &str,
        format_tag: u16,
        bits: u16,
        channels: u16,
        data: Vec<u8>,
    ) -> SoundData {
        let block_align = channels * bits / 8;
        SoundData {
            name: name.to_string(),
            path: format!("C:\\sounds\\{name}.{ext}"),
            wave_form: WaveForm {
                format_tag,
                channels,
                samples_per_sec: 44100,
                avg_bytes_per_sec: 44100 * u32::from(block_align),
                block_align,
                bits_per_sample: bits,
                cb_size: 0,
            },
            data,
            internal_name: String::new(),
            fade: 0,
            volume: 100,
            balance: 0,
            output_target: OutputTarget::Table,
        }
    }

    fn le16(samples: &[i16]) -> Vec<u8> {
        samples.iter().flat_map(|s| s.to_le_bytes()).collect()
    }

    #[test]
    fn sixteen_bit_stereo_is_averaged() -> TestResult {
        // frames: (100, 200), (-100, 101), (32767, 32767), (-32768, 0), plus a
        // trailing partial frame that is dropped
        let mut data = le16(&[100, 200, -100, 101, 32767, 32767, -32768, 0]);
        data.push(7);
        let mut s = sound("hit", "wav", 1, 16, 2, data);
        assert!(s.to_mono()?);
        assert_eq!(s.data, le16(&[150, 1, 32767, -16384]));
        assert_eq!(s.wave_form.channels, 1);
        assert_eq!(s.wave_form.block_align, 2);
        assert_eq!(s.wave_form.avg_bytes_per_sec, 88200);
        assert!(!s.to_mono()?);
        Ok(())
    }

    #[test]
    fn eight_bit_unsigned_stays_centered() -> TestResult {
        // 128 is silence; (128, 128) -> 128, (0, 255) averages to -0.5 which
        // rounds away from zero to 127, (255, 255) -> 255
        let mut s = sound("click", "wav", 1, 8, 2, vec![128, 128, 0, 255, 255, 255]);
        assert!(s.to_mono()?);
        assert_eq!(s.data, vec![128, 127, 255]);
        Ok(())
    }

    #[test]
    fn twenty_four_and_thirty_two_bit_are_averaged() -> TestResult {
        let i24 = |v: i32| v.to_le_bytes()[..3].to_vec();
        let data: Vec<u8> = [
            i24(1_000_000),
            i24(-2_000_000),
            i24(-8_388_608),
            i24(8_388_607),
        ]
        .concat();
        let mut s = sound("thud", "wav", 1, 24, 2, data);
        assert!(s.to_mono()?);
        assert_eq!(s.data, [i24(-500_000), i24(-1)].concat());

        let data: Vec<u8> = [3i32, 4, -1_000_000_000, 1_000_000_001]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut s = sound("deep", "wav", 1, 32, 2, data);
        assert!(s.to_mono()?);
        assert_eq!(
            s.data,
            [4i32, 1]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect::<Vec<u8>>()
        );
        Ok(())
    }

    #[test]
    fn float_and_four_channels_are_averaged() -> TestResult {
        let data: Vec<u8> = [0.5f32, -0.5, 1.0, 0.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let mut s = sound("quad", "wav", 3, 32, 4, data);
        assert!(s.to_mono()?);
        assert_eq!(s.data, 0.25f32.to_le_bytes());
        assert_eq!(s.wave_form.block_align, 4);
        Ok(())
    }

    #[test]
    fn other_formats_and_files_are_left_alone() {
        let mut s = sound("music", "mp3", 1, 16, 2, vec![0; 8]);
        assert!(!s.to_mono().expect("no error for an mp3"));
        assert_eq!(s.wave_form.channels, 2);

        let mut s = sound("weird", "wav", 0xFFFE, 16, 2, vec![0; 8]);
        let error = s.to_mono().expect_err("extensible is not mixed");
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
        assert_eq!(s.wave_form.channels, 2);
    }

    #[test]
    fn a_table_converts_its_playfield_sounds_and_round_trips() -> TestResult {
        let mut table = vpx::from_bytes(include_bytes!(
            "../../testdata/completely_blank_table_10_7_4.vpx"
        ))?;
        let mut backglass = sound("music", "wav", 1, 16, 2, le16(&[1, 2, 3, 4]));
        backglass.output_target = OutputTarget::Backglass;
        table.sounds = vec![
            sound("hit", "wav", 1, 16, 2, le16(&[100, 200])),
            sound("mono", "wav", 1, 16, 1, le16(&[5])),
            backglass,
            sound("ogg", "ogg", 1, 16, 2, vec![1, 2, 3, 4]),
        ];
        table.gamedata.sounds_size = 4;
        let results = table.playfield_sounds_to_mono();
        assert_eq!(
            results,
            vec![
                SoundToMono::Converted {
                    name: "hit".to_string(),
                    channels: 2,
                    bytes_before: 4,
                    bytes_after: 2,
                },
                SoundToMono::Skipped {
                    name: "ogg".to_string(),
                    reason: MonoSkipReason::NotWav("ogg".to_string()),
                },
            ]
        );
        assert_eq!(
            results[0].to_string(),
            "sound \"hit\": 2 channels -> mono, 4 -> 2 bytes"
        );
        assert_eq!(table.sounds[2].wave_form.channels, 2);

        let written = vpx::to_bytes(&table)?;
        let read = vpx::from_bytes(&written)?;
        assert_eq!(read.sounds[0].wave_form, table.sounds[0].wave_form);
        assert_eq!(read.sounds[0].data, le16(&[150]));
        Ok(())
    }
}
