use super::{
    Version,
    biff::{BiffError, BiffReader, BiffWriter},
};
use crate::vpx::wav::{WavHeader, read_wav_header, write_wav_header};
use bytes::{BufMut, BytesMut};
use log::warn;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io;
use std::path::Path;
use tracing::instrument;

/// Audio device a sound plays on, mirroring vpinball's `SoundOutTypes`.
///
/// Values this library does not know are kept in [`OutputTarget::Other`] so the
/// table round-trips unchanged; reading one logs a warning.
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum OutputTarget {
    /// `SNDOUT_TABLE`: the table (playfield) audio device.
    Table,
    /// `SNDOUT_BACKGLASS`: the backglass (music) audio device.
    Backglass,
    /// A value not known to this library, kept as is.
    ///
    /// Must not be constructed with a value that maps to a named variant
    /// (0 or 1): it would write the same bytes as the named variant and
    /// read back as it, breaking round-trip equality. The library itself
    /// never does (`From` normalizes known values to their named
    /// variants), and the test strategy is constrained to the genuinely
    /// unknown range for the same reason.
    Other(#[cfg_attr(test, proptest(strategy = "2..=u8::MAX"))] u8),
}
impl From<u8> for OutputTarget {
    fn from(value: u8) -> Self {
        match value {
            0 => OutputTarget::Table,
            1 => OutputTarget::Backglass,
            other => {
                warn!("Unknown OutputTarget value {other}, keeping it as is");
                OutputTarget::Other(other)
            }
        }
    }
}
impl From<&OutputTarget> for u8 {
    fn from(value: &OutputTarget) -> Self {
        match value {
            OutputTarget::Table => 0,
            OutputTarget::Backglass => 1,
            OutputTarget::Other(value) => *value,
        }
    }
}
/// Serialize to lowercase string, or the raw number for [`OutputTarget::Other`]
impl Serialize for OutputTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            OutputTarget::Table => serializer.serialize_str("table"),
            OutputTarget::Backglass => serializer.serialize_str("backglass"),
            OutputTarget::Other(value) => serializer.serialize_u8(*value),
        }
    }
}
/// Deserialize from lowercase string, or from the raw number
impl<'de> Deserialize<'de> for OutputTarget {
    fn deserialize<D>(deserializer: D) -> Result<OutputTarget, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct OutputTargetVisitor;
        impl serde::de::Visitor<'_> for OutputTargetVisitor {
            type Value = OutputTarget;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a OutputTarget as lowercase string or number")
            }
            fn visit_u64<E>(self, value: u64) -> Result<OutputTarget, E>
            where
                E: serde::de::Error,
            {
                let value = u8::try_from(value).map_err(|_| {
                    serde::de::Error::invalid_value(
                        serde::de::Unexpected::Unsigned(value),
                        &"a number that fits in u8",
                    )
                })?;
                Ok(OutputTarget::from(value))
            }
            fn visit_str<E>(self, value: &str) -> Result<OutputTarget, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "table" => Ok(OutputTarget::Table),
                    "backglass" => Ok(OutputTarget::Backglass),
                    _ => Err(serde::de::Error::unknown_variant(
                        value,
                        &["table", "backglass"],
                    )),
                }
            }
        }
        deserializer.deserialize_any(OutputTargetVisitor)
    }
}
#[cfg(test)]
mod output_target_open_enum_tests {
    use super::OutputTarget;

    #[test]
    fn unknown_value_round_trips() {
        let value = OutputTarget::from(250);
        assert_eq!(value, OutputTarget::Other(250));
        assert_eq!(u8::from(&value), 250);
        let json = serde_json::to_value(value.clone()).unwrap();
        assert_eq!(json, serde_json::json!(250u8));
        let back: OutputTarget = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
        assert!(
            serde_json::from_value::<OutputTarget>(serde_json::json!("no_such_variant")).is_err()
        );
    }
}

const NEW_SOUND_FORMAT_VERSION: u32 = 1031;

impl fmt::Debug for SoundData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // avoid writing the data to the debug output
        f.debug_struct("SoundData")
            .field("name", &self.name)
            .field("path", &self.path)
            .field("wave_form", &self.wave_form)
            .field("data", &self.data.len())
            .field("internal_name", &self.internal_name)
            .field("fade", &self.fade)
            .field("volume", &self.volume)
            .field("balance", &self.balance)
            .field("output_target", &self.output_target)
            .finish()
    }
}

/// A sound of the table, mirroring vpinball's `VPX::Sound`.
///
/// vpinball stores every sound as its own `Sound<n>` stream in the table
/// storage. Unlike most of the file it is not a tagged BIFF record but a
/// fixed sequence of fields (`Sound::CreateFromStream` and
/// `Sound::SaveToStream` in `src/parts/Sound.cpp`): the length-prefixed
/// [`SoundData::name`], [`SoundData::path`] and [`SoundData::internal_name`],
/// for WAV files the [`WaveForm`] header, the length-prefixed
/// [`SoundData::data`], one byte for [`SoundData::output_target`] and, in
/// files of version 1031 (`NEW_SOUND_FORMAT_VERSION`) and newer, the volume,
/// balance, fade and once more the volume as 32-bit integers.
#[derive(PartialEq)]
pub struct SoundData {
    /// Name of the sound, as shown in the sound manager and passed to
    /// `PlaySound` by table scripts (`Sound::m_name`).
    ///
    /// Stored as a length-prefixed string.
    pub name: String,
    /// Path of the file the sound was imported from (`Sound::m_path`).
    ///
    /// vpinball only uses its extension: a `.wav` file is stored as a
    /// [`WaveForm`] header plus the raw samples, any other format as the
    /// unchanged file bytes. Stored as a length-prefixed string.
    pub path: String,
    /// The `WAVEFORMATEX` header of a WAV sound.
    ///
    /// Only present in the file when [`SoundData::path`] has a `.wav`
    /// extension (this library also assumes WAV for a path without
    /// extension); vpinball rebuilds the 44-byte RIFF header from it on
    /// load. For other formats nothing is stored and this holds
    /// [`WaveForm::default`].
    pub wave_form: WaveForm,
    /// The audio data.
    ///
    /// For WAV sounds the raw sample bytes without the RIFF header (vpinball
    /// strips the header on save and rebuilds it on load); for any other
    /// format the complete original file. Stored with a 32-bit length prefix.
    pub data: Vec<u8>,
    /// Removed: previously did write the same name again, but just in lower case
    /// This rudimentary version here needs to stay as otherwise problems when loading, as one field less
    /// Now just writes a short dummy/empty string.
    /// see <https://github.com/vpinball/vpinball/commit/3320dd11d66ecedba326197c7d4e85c48864cc19>
    pub internal_name: String,
    /// Front/rear fade of the sound on the playfield speakers
    /// (`Sound::m_frontRearFade`).
    ///
    /// Percent in the range -100 (rear) to 100 (front); vpinball maps it to
    /// -1..1 and adds the fade passed to `PlaySound`. Only used for the
    /// table output target. vpinball stores it as a signed 32-bit integer;
    /// this library keeps the raw bits, so a negative setting shows as a
    /// large value. Absent in files older than 1031, where vpinball uses 100
    /// and this library reads 0.
    pub fade: u32,
    /// Volume of the sound (`Sound::m_volume`).
    ///
    /// Percent in the range -100 to 100; vpinball maps it to -1..1 and adds
    /// the volume passed to `PlaySound`. Stored twice in the stream, once
    /// before the balance and once after the fade. vpinball stores it as a
    /// signed 32-bit integer; this library keeps the raw bits, so a negative
    /// setting shows as a large value. Absent in files older than 1031, where
    /// vpinball uses 100 and this library reads 0.
    pub volume: u32,
    /// Left/right balance, the pan of the sound (`Sound::m_pan`).
    ///
    /// Percent in the range -100 (left) to 100 (right); vpinball maps it to
    /// -1..1 and adds the pan passed to `PlaySound`. vpinball stores it as a
    /// signed 32-bit integer; this library keeps the raw bits, so a negative
    /// setting shows as a large value. Absent in files older than 1031, where
    /// vpinball uses 100 and this library reads 0.
    pub balance: u32,
    /// The audio device the sound plays on (`Sound::m_outputTarget`).
    ///
    /// Stored as one byte. In files older than 1031 the same byte is a
    /// "to backglass output" bool, and vpinball additionally treats a name
    /// containing `bgout_` or the path `* Backglass Output *` as backglass.
    /// Default: [`OutputTarget::Table`].
    pub output_target: OutputTarget,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SoundDataJson {
    name: String,
    path: String,
    /// Not used by vpinball, kept for binary format compatibility.
    /// Optional as other tools writing the json might not provide it.
    #[serde(default)]
    internal_name: String,
    fade: u32,
    volume: u32,
    balance: u32,
    output_target: OutputTarget,
    /// In case we have a duplicate name or the file name is not simply derived from the name
    /// because of special characters etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name_dedup: Option<String>,
}

impl SoundDataJson {
    pub fn from_sound_data(sound_data: &SoundData) -> Self {
        Self {
            name: sound_data.name.clone(),
            path: sound_data.path.clone(),
            internal_name: sound_data.internal_name.clone(),
            fade: sound_data.fade,
            volume: sound_data.volume,
            balance: sound_data.balance,
            output_target: sound_data.output_target.clone(),
            name_dedup: None,
        }
    }
    pub fn to_sound_data(&self) -> SoundData {
        SoundData {
            name: self.name.clone(),
            path: self.path.clone(),
            // this is populated by reading the wav or default for other files
            wave_form: WaveForm::default(),
            data: Vec::new(),
            internal_name: self.internal_name.clone(),
            fade: self.fade,
            volume: self.volume,
            balance: self.balance,
            output_target: self.output_target.clone(),
        }
    }
}

const WAV_HEADER_SIZE: usize = 44;

fn write_wav_header2(sound_data: &SoundData) -> Vec<u8> {
    let data_len = if sound_data.wave_form.format_tag == 1 {
        // In the vpx file for PCM this is always 0,
        // so we use the length of the data.
        sound_data.data.len() as u32 // 4
    } else {
        sound_data.wave_form.cb_size as u32 // 4
    };
    //let bytes_per_sec = sound_data.wave_form.avg_bytes_per_sec;
    let bytes_per_sec = sound_data.wave_form.samples_per_sec
        * sound_data.wave_form.bits_per_sample as u32
        * sound_data.wave_form.channels as u32
        / 8;
    let extension_size = if sound_data.wave_form.format_tag == 1 {
        None
    } else {
        Some(0)
    };
    // a fmt chunk with a cbSize field is 18 bytes instead of 16
    let fmt_size = if extension_size.is_some() { 18 } else { 16 };

    let wav_header = WavHeader {
        size: sound_data.data.len() as u32 + 36,
        fmt_size,
        format_tag: sound_data.wave_form.format_tag,
        channels: sound_data.wave_form.channels,
        samples_per_sec: sound_data.wave_form.samples_per_sec,
        avg_bytes_per_sec: bytes_per_sec,
        block_align: sound_data.wave_form.block_align,
        bits_per_sample: sound_data.wave_form.bits_per_sample,
        extension_size,
        extension_fields: Vec::new(),
        pre_fmt_fields: Vec::new(),
        extra_fields: Vec::new(),
        data_size: data_len,
    };
    let mut buf = BytesMut::with_capacity(WAV_HEADER_SIZE);
    write_wav_header(&wav_header, &mut buf);
    buf.to_vec() // total 44 bytes
}

impl From<WavHeader> for WaveForm {
    fn from(header: WavHeader) -> Self {
        WaveForm {
            format_tag: header.format_tag,
            channels: header.channels,
            samples_per_sec: header.samples_per_sec,
            avg_bytes_per_sec: header.avg_bytes_per_sec,
            block_align: header.block_align,
            bits_per_sample: header.bits_per_sample,
            cb_size: 0,
        }
    }
}

/// Rebuilds the sound file as it was imported.
///
/// For a WAV sound (by the extension of [`SoundData::path`]) this is a
/// 44-byte RIFF/WAVE header built from [`SoundData::wave_form`] followed by
/// [`SoundData::data`], the way vpinball does on load. For any other format
/// it is [`SoundData::data`] unchanged.
pub fn write_sound(sound_data: &SoundData) -> Vec<u8> {
    if is_wav(&sound_data.path) {
        let mut buf = BytesMut::with_capacity(WAV_HEADER_SIZE + sound_data.data.len());
        buf.put_slice(&write_wav_header2(sound_data));
        buf.put_slice(&sound_data.data);
        buf.to_vec()
    } else {
        sound_data.data.clone()
    }
}

/// Fills `sound_data` from the bytes of a sound file, the inverse of
/// [`write_sound`].
///
/// For a WAV sound (by the extension of [`SoundData::path`]) the RIFF header
/// is parsed into [`SoundData::wave_form`] and the remaining bytes become
/// [`SoundData::data`]; for any other format the whole file becomes
/// [`SoundData::data`]. Fails when a WAV header cannot be parsed.
pub fn read_sound(data: &[u8], sound_data: &mut SoundData) -> io::Result<()> {
    if is_wav(&sound_data.path) {
        let mut reader = bytes::BytesMut::from(data);
        let header = read_wav_header(&mut reader).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "Failed to read wav header for sound '{}': {e}",
                    sound_data.name
                ),
            )
        })?;
        let header_data_size = header.data_size;
        // read all remaining bits
        sound_data.data = reader.to_vec();
        let mut wave_form: WaveForm = header.into();
        if wave_form.format_tag == 1 {
            // in the vpx file this is always 0 for PCM
            wave_form.cb_size = 0;
        } else {
            wave_form.cb_size = header_data_size as u16;
        }
        sound_data.wave_form = wave_form;
    } else {
        sound_data.data = data.to_vec();
    }
    Ok(())
}

/// The WAV file vpinball builds when it loads a `.wav` sound from a
/// `.vpx`: a 44 byte RIFF header from the stored [`WaveForm`], then the
/// samples (`Sound::CreateFromStream`). This is the file a VPZ pack holds.
///
/// Unlike [`write_sound`], which keeps every stored header field so that
/// an extracted table assembles byte for byte, this drops `cbSize` as
/// vpinball does.
pub(crate) fn vpinball_wav_file(wave_form: &WaveForm, samples: &[u8]) -> Vec<u8> {
    let mut wav = Vec::with_capacity(VPINBALL_WAV_HEADER_SIZE + samples.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&wave_form.format_tag.to_le_bytes());
    wav.extend_from_slice(&wave_form.channels.to_le_bytes());
    wav.extend_from_slice(&wave_form.samples_per_sec.to_le_bytes());
    wav.extend_from_slice(&wave_form.avg_bytes_per_sec.to_le_bytes());
    wav.extend_from_slice(&wave_form.block_align.to_le_bytes());
    wav.extend_from_slice(&wave_form.bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    wav.extend_from_slice(samples);
    wav
}

/// The [`WaveForm`] and samples vpinball saves from a WAV file: the
/// format fields at the offsets of its 44 byte header, `cbSize` 0, and
/// everything after the header as samples (`Sound::SaveToStream`). The
/// inverse of [`vpinball_wav_file`].
///
/// Unlike [`read_sound`], which parses the chunks of any WAV and takes
/// `cbSize` from the data chunk size where [`write_sound`] keeps it, this
/// reads a file as vpinball does, whatever its chunks.
pub(crate) fn read_vpinball_wav(data: &[u8]) -> io::Result<(WaveForm, Vec<u8>)> {
    let header = data.get(..VPINBALL_WAV_HEADER_SIZE).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("a WAV file of {} bytes has no 44 byte header", data.len()),
        )
    })?;
    let u16_at = |o: usize| u16::from_le_bytes([header[o], header[o + 1]]);
    let u32_at =
        |o: usize| u32::from_le_bytes([header[o], header[o + 1], header[o + 2], header[o + 3]]);
    let wave_form = WaveForm {
        format_tag: u16_at(20),
        channels: u16_at(22),
        samples_per_sec: u32_at(24),
        avg_bytes_per_sec: u32_at(28),
        block_align: u16_at(32),
        bits_per_sample: u16_at(34),
        cb_size: 0,
    };
    Ok((wave_form, data[VPINBALL_WAV_HEADER_SIZE..].to_vec()))
}

/// The size of the RIFF header vpinball writes and expects
const VPINBALL_WAV_HEADER_SIZE: usize = 44;

/// Format of the samples of a WAV sound, mirroring the Windows `WAVEFORMATEX`
/// struct.
///
/// vpinball, originally Windows only, stores a `.wav` sound as this 18-byte
/// little-endian struct followed by the raw samples instead of the file's
/// RIFF header (`Sound::SaveToStream` in `src/parts/Sound.cpp`). The fields
/// are described as Microsoft defines them.
#[derive(Debug, PartialEq)]
pub struct WaveForm {
    /// `wFormatTag`: the waveform-audio format type. `1` is
    /// `WAVE_FORMAT_PCM`, uncompressed PCM, the usual case.
    pub format_tag: u16,
    /// `nChannels`: the number of channels; 1 for mono, 2 for stereo.
    pub channels: u16,
    /// `nSamplesPerSec`: the sample rate in samples per second (Hz), for
    /// example 44100.
    pub samples_per_sec: u32,
    /// `nAvgBytesPerSec`: the required average data transfer rate in bytes
    /// per second, used for buffer estimation. For PCM this is
    /// [`WaveForm::samples_per_sec`] x [`WaveForm::block_align`].
    pub avg_bytes_per_sec: u32,
    /// `nBlockAlign`: the block alignment in bytes, the minimum atomic unit
    /// of data. For PCM this is [`WaveForm::channels`] x
    /// [`WaveForm::bits_per_sample`] / 8.
    pub block_align: u16,
    /// `wBitsPerSample`: the bits per sample of one channel. For PCM this is
    /// 8 or 16.
    pub bits_per_sample: u16,
    /// `cbSize`: the size in bytes of extra format information that follows
    /// the struct.
    ///
    /// vpinball always writes 0 and ignores the value on load. This library
    /// reuses the field for non-PCM formats: [`read_sound`] stores the size
    /// of the WAV `data` chunk here and [`write_sound`] writes it back, as
    /// that size cannot be derived from the data length for such formats.
    pub cb_size: u16,
}

impl WaveForm {
    /// A header for 16-bit mono PCM at 44100 Hz: block align 2, 88200 bytes
    /// per second, no extra data.
    ///
    /// Used for sounds that are not WAV files, for which nothing is stored.
    pub fn new() -> WaveForm {
        WaveForm {
            format_tag: 1,
            channels: 1,
            samples_per_sec: 44100,
            avg_bytes_per_sec: 88200,
            block_align: 2,
            bits_per_sample: 16,
            cb_size: 0,
        }
    }
}

impl Default for WaveForm {
    fn default() -> Self {
        Self::new()
    }
}

/// The format the content of a stored sound file names, by its signature,
/// as the short lower case name the audit shows: `wav`, `ogg`, `mp3` and
/// `flac`, the formats vpinball's decoder reads. `None` when no signature
/// matches. Only meaningful for a sound stored as a file, that is one
/// whose path does not name a `.wav`: a wav is stored as a header plus
/// raw samples and carries no signature.
pub(crate) fn content_format(data: &[u8]) -> Option<&'static str> {
    let format = if data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WAVE") {
        "wav"
    } else if data.starts_with(b"OggS") {
        "ogg"
    } else if data.starts_with(b"fLaC") {
        "flac"
    } else if data.starts_with(b"ID3")
        || (data.len() >= 2 && data[0] == 0xFF && data[1] & 0xE0 == 0xE0)
    {
        // an ID3 tag, or straight into the first frame's sync word
        "mp3"
    } else {
        return None;
    };
    Some(format)
}

/// The channel count of a sound stored as a file, read from its header:
/// the `fmt ` chunk of a RIFF wave, FLAC's STREAMINFO, the Vorbis
/// identification header of an Ogg file, or the first MPEG audio frame of
/// an MP3 (1 for mono, 2 otherwise). `None` for a file this cannot read the
/// channels of.
pub(crate) fn file_channels(data: &[u8]) -> Option<u16> {
    match content_format(data)? {
        "wav" => riff_channels(data),
        "flac" => flac_channels(data),
        "ogg" => vorbis_channels(data),
        "mp3" => mp3_channels(data),
        _ => None,
    }
}

/// The channels of a RIFF wave, from its `fmt ` chunk
fn riff_channels(data: &[u8]) -> Option<u16> {
    let mut offset = 12;
    loop {
        let id = data.get(offset..offset + 4)?;
        let size = u32::from_le_bytes(data.get(offset + 4..offset + 8)?.try_into().ok()?) as usize;
        if id == b"fmt " {
            let channels = data.get(offset + 10..offset + 12)?;
            return Some(u16::from_le_bytes([channels[0], channels[1]]));
        }
        // chunks are padded to an even size
        offset = offset.checked_add(8 + size + (size & 1))?;
    }
}

/// The channels of a FLAC stream: STREAMINFO is the first metadata block,
/// its channel count minus one is 3 bits after the 20 bit sample rate
fn flac_channels(data: &[u8]) -> Option<u16> {
    // "fLaC", the 4 byte block header, then 10 bytes of block and frame
    // sizes before the sample rate
    if data.get(4)? & 0x7F != 0 {
        return None;
    }
    Some(u16::from((data.get(20)? >> 1) & 0x07) + 1)
}

/// The channels of the first stream of an Ogg file, when it is Vorbis: the
/// first page holds the identification header, `\x01vorbis`, a 4 byte
/// version, then the channel count
fn vorbis_channels(data: &[u8]) -> Option<u16> {
    let segments = usize::from(*data.get(26)?);
    let packet = data.get(27 + segments..)?;
    if !packet.starts_with(b"\x01vorbis") {
        return None;
    }
    let channels = *packet.get(11)?;
    (channels > 0).then_some(u16::from(channels))
}

/// The channels of the first MPEG audio frame of an MP3, after an ID3v2
/// tag when there is one: channel mode 3 is mono, the others two channels
fn mp3_channels(data: &[u8]) -> Option<u16> {
    let mut offset = 0;
    if data.starts_with(b"ID3") {
        let size = data
            .get(6..10)?
            .iter()
            .fold(0usize, |size, byte| (size << 7) | usize::from(byte & 0x7F));
        let footer = if data.get(5)? & 0x10 != 0 { 10 } else { 0 };
        offset = 10 + size + footer;
    }
    // the first plausible frame header: sync word, a layer, a bitrate and a
    // sample rate that are not reserved
    let rest = data.get(offset..)?;
    let header = rest.windows(4).find(|header| {
        header[0] == 0xFF
            && header[1] & 0xE0 == 0xE0
            && header[1] & 0x18 != 0x08
            && header[1] & 0x06 != 0
            && header[2] & 0xF0 != 0xF0
            && header[2] & 0x0C != 0x0C
    })?;
    Some(if header[3] >> 6 == 3 { 1 } else { 2 })
}

/// The format a sound's file extension names, in the same short names as
/// [`content_format`]. `None` for an extension that names no audio format
/// vpinball reads.
pub(crate) fn extension_format(extension: &str) -> Option<&'static str> {
    let format = match extension.to_ascii_lowercase().as_str() {
        "wav" => "wav",
        "ogg" => "ogg",
        "mp3" => "mp3",
        "flac" => "flac",
        _ => return None,
    };
    Some(format)
}

impl SoundData {
    /// Whether this sound is a WAV, stored as a [`WaveForm`] header plus
    /// raw samples rather than a file. True when the path names a `.wav`
    /// or has no extension, as vpinball treats it.
    ///
    /// Only the native wav-to-flac fix uses this, so it is gated to match.
    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn is_wav(&self) -> bool {
        is_wav(&self.path)
    }

    /// The extension of [`SoundData::path`], as written, or `None` when the
    /// path has none
    pub(crate) fn extension(&self) -> Option<&str> {
        str_path_ext(&self.path)
    }

    pub(crate) fn ext(&self) -> String {
        match str_path_ext(&self.path) {
            Some(ext) => ext.to_string(),
            None => {
                warn!(
                    "Sound path '{}' has no extension, assuming 'wav'",
                    self.path
                );
                "wav".to_string()
            }
        }
    }
}

#[instrument(skip(file_version, reader))]
pub(crate) fn read(
    file_version: &Version,
    reader: &mut BiffReader,
) -> Result<SoundData, BiffError> {
    let mut name: String = "".to_string();
    let mut path: String = "".to_string();
    let mut internal_name: String = "".to_string();
    let mut fade: u32 = 0;
    let mut volume: u32 = 0;
    let mut balance: u32 = 0;
    let mut output_target: OutputTarget = OutputTarget::Table;
    let mut data: Vec<u8> = Vec::new();
    let mut wave_form: WaveForm = WaveForm::new();

    // TODO add support for the old format file version < 1031
    // https://github.com/freezy/VisualPinball.Engine/blob/ec1e9765cd4832c134e889d6e6d03320bc404bd5/VisualPinball.Engine/VPT/Sound/SoundData.cs#L98

    let num_values = if file_version.u32() < NEW_SOUND_FORMAT_VERSION {
        6
    } else {
        10
    };

    // We have seen below case for a 1040 file:
    // Legacy behavior, where the BG selection was encoded into the strings directly
    // path = "* Backglass Output *" or name contains "bgout_"
    // This is still seen as a wav file, even if the path does not end with .wav!

    for i in 0..num_values {
        match i {
            0 => {
                name = reader.get_string_no_remaining_update()?;
            }
            1 => {
                path = reader.get_string_no_remaining_update()?;
            }
            2 => {
                internal_name = reader.get_string_no_remaining_update()?;
            }
            3 => {
                if is_wav(&path.to_owned()) {
                    wave_form = read_wave_form(reader)?;
                } else {
                    // should we be doing something here?
                }
            }
            4 => {
                data = reader.get_data_no_remaining_update()?;
            }
            5 => {
                output_target = reader.get_u8_no_remaining_update()?.into();
            }
            6 => {
                volume = reader.get_u32_no_remaining_update()?;
            }
            7 => {
                balance = reader.get_u32_no_remaining_update()?;
            }
            8 => {
                fade = reader.get_u32_no_remaining_update()?;
            }
            9 => {
                // TODO why do we have the volume twice?
                volume = reader.get_u32_no_remaining_update()?;
            }
            unexpected => {
                return Err(reader.err(format!("unexpected sound field {unexpected}")));
            }
        }
    }

    Ok(SoundData {
        name,
        path,
        data: data.to_vec(),
        wave_form,
        internal_name,
        fade,
        volume,
        balance,
        output_target,
    })
}

fn str_path_ext(path: &str) -> Option<&str> {
    Path::new(path)
        .extension()
        .and_then(|os| os.to_str())
        .filter(|s| !s.is_empty())
}

/// Check if the path is a wav file.
/// If the path does not have an extension, it is also considered a wav file!
pub(crate) fn is_wav(path: &str) -> bool {
    match str_path_ext(path) {
        Some(ext) => ext.eq_ignore_ascii_case("wav"),
        None => true,
    }
}

/// The outcome of [`SoundData::wav_to_flac`] for a sound that was a
/// candidate: a PCM WAV. Mirrors [`crate::vpx::images::Webp`].
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Flac {
    /// Re-encoded as FLAC, the sound now holds the smaller `.flac` file
    Converted,
    /// The FLAC would not be smaller than the stored WAV samples, left as is
    NotSmaller,
}

impl SoundData {
    /// Re-encodes a PCM WAV sound as FLAC when that is smaller. FLAC is
    /// lossless, so the samples play back the same, but only vpinball
    /// builds with the miniaudio sound engine (10.8.1 and later) decode
    /// FLAC, which is why this is an opt-in size lever rather than a
    /// repair.
    ///
    /// Returns `None` when the sound is not a PCM WAV (a non-WAV file, or
    /// a WAV whose `format_tag` is not 1, such as ADPCM or float), which
    /// cannot be re-encoded here. For a candidate it answers whether the
    /// FLAC replaced the samples or was not smaller.
    ///
    /// # Errors
    ///
    /// When the samples cannot be encoded as FLAC.
    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn wav_to_flac(&mut self) -> io::Result<Option<Flac>> {
        if !is_wav(&self.path) || self.wave_form.format_tag != 1 {
            return Ok(None);
        }
        let bits = self.wave_form.bits_per_sample;
        let channels = self.wave_form.channels;
        if channels == 0 || !matches!(bits, 8 | 16 | 24) {
            // outside what the deinterleaver below handles; leave it be
            return Ok(None);
        }
        let samples = pcm_samples(&self.data, bits);
        let bytes_before = self.data.len();
        let flac = encode_flac(&samples, channels, bits, self.wave_form.samples_per_sec)?;

        if flac.len() >= bytes_before {
            return Ok(Some(Flac::NotSmaller));
        }
        self.data = flac;
        self.path = replace_sound_extension(&self.path, "flac");
        // no longer a wav, so the stored header is irrelevant
        self.wave_form = WaveForm::default();
        Ok(Some(Flac::Converted))
    }
}

/// Encodes interleaved samples of the given depth as a FLAC file
#[cfg(not(target_family = "wasm"))]
fn encode_flac(
    samples: &[i32],
    channels: u16,
    bits_per_sample: u16,
    sample_rate: u32,
) -> io::Result<Vec<u8>> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| io::Error::other(format!("flac config: {e:?}")))?;
    let source = flacenc::source::MemSource::from_samples(
        samples,
        channels as usize,
        bits_per_sample as usize,
        sample_rate as usize,
    );
    let mut stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| io::Error::other(format!("flac encode: {e:?}")))?;
    // Published flacenc 0.5.1 sets min == max block size up front, which is wrong when
    // the stream ends in a shorter tail frame; strict decoders (miniaudio) then reject it.
    // Re-set min = max = max_block_size after encoding, matching yotarok/flacenc-rs#255.
    let max_block_size = stream.stream_info().max_block_size();
    stream
        .stream_info_mut()
        .set_block_sizes(max_block_size, max_block_size)
        .map_err(|e| io::Error::other(format!("flac block sizes: {e:?}")))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| io::Error::other(format!("flac write: {e:?}")))?;
    Ok(sink.into_inner())
}

/// What [`SoundData::downmix_to_mono`] did with a sound of more than one channel
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Mono {
    /// Downmixed, the sound now holds one channel
    Converted,
    /// A wav that is not 8, 16, 24 or 32 bit PCM or 32 bit float
    NotPcm,
    /// An MP3 or Ogg file, which re-encoding would degrade
    Lossy,
    /// A FLAC of 24 bits or more: the exact mono needs a bit more, past
    /// what the encoder writes
    TooDeep,
    /// The mono FLAC would not be smaller
    NotSmaller,
    /// Something else this does not downmix, why
    Unsupported(String),
}

#[cfg(not(target_family = "wasm"))]
impl SoundData {
    /// Downmixes a sound of more than one channel to the mono vpinball
    /// plays it as on the playfield: miniaudio 0.11, which vpinball 10.8.1
    /// and later decode with, averages the channels when a playfield sound
    /// is decoded to one channel (`ma_channel_mix_mode_simple`), see
    /// [`downmix_wav`].
    ///
    /// A wav keeps its sample format with one channel. A two channel FLAC
    /// up to 23 bits becomes a mono FLAC one bit deeper holding the sum of
    /// the channels: miniaudio averages FLAC samples as floats, which only
    /// that depth stores exactly. MP3 and Ogg files are lossy and left
    /// alone.
    pub(crate) fn downmix_to_mono(&mut self) -> io::Result<Mono> {
        if is_wav(&self.path) {
            let Some(data) = downmix_wav(&self.data, &self.wave_form) else {
                return Ok(Mono::NotPcm);
            };
            let bytes_per_sample = self.wave_form.bits_per_sample / 8;
            self.data = data;
            self.wave_form.channels = 1;
            self.wave_form.block_align = bytes_per_sample;
            self.wave_form.avg_bytes_per_sec =
                self.wave_form.samples_per_sec * u32::from(bytes_per_sample);
            return Ok(Mono::Converted);
        }
        match content_format(&self.data) {
            Some("flac") => self.flac_to_mono(),
            Some("mp3" | "ogg") => Ok(Mono::Lossy),
            Some(format) => Ok(Mono::Unsupported(format!(
                "a {format} stored under another name"
            ))),
            None => Ok(Mono::Unsupported("an unknown format".to_string())),
        }
    }

    fn flac_to_mono(&mut self) -> io::Result<Mono> {
        let mut reader =
            claxon::FlacReader::new(io::Cursor::new(&self.data)).map_err(io::Error::other)?;
        let info = reader.streaminfo();
        if info.channels != 2 {
            return Ok(Mono::Unsupported(format!(
                "a {} channel flac",
                info.channels
            )));
        }
        let bits = info.bits_per_sample as u16;
        if bits >= 24 {
            return Ok(Mono::TooDeep);
        }
        if bits < 7 {
            return Ok(Mono::Unsupported(format!("a {bits} bit flac")));
        }
        let samples = reader
            .samples()
            .collect::<Result<Vec<i32>, _>>()
            .map_err(io::Error::other)?;
        let mono: Vec<i32> = samples
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[left, right]| left + right)
            .collect();
        let flac = encode_flac(&mono, 1, bits + 1, info.sample_rate)?;
        if flac.len() >= self.data.len() {
            return Ok(Mono::NotSmaller);
        }
        self.data = flac;
        Ok(Mono::Converted)
    }
}

/// miniaudio's `0.00784313725490196078f`, 2/255 as the nearest float
#[cfg(not(target_family = "wasm"))]
const U8_TO_F32: f32 = f32::from_bits(0x3C00_8081);
/// miniaudio's `0.00000011920928955078125f`, 2^-23
#[cfg(not(target_family = "wasm"))]
const S24_TO_F32: f32 = f32::from_bits(0x3400_0000);

/// Averages the channels of interleaved wav samples into one, as miniaudio
/// 0.11 does when vpinball decodes a playfield sound to mono: in the
/// samples' own format for 16 bit PCM and 32 bit float, through floats for
/// 8, 24 and 32 bit PCM (`ma_data_converter_config_get_mid_format`). The
/// sum of the channels is divided by their count
/// (`ma_channel_converter_process_pcm_frames__mono_out`), and the sample
/// conversions are miniaudio's reference ones, which its SIMD variants
/// call too. `None` for any other format. A trailing partial frame is
/// dropped, as the decoder reads whole frames.
#[cfg(not(target_family = "wasm"))]
fn downmix_wav(data: &[u8], wave_form: &WaveForm) -> Option<Vec<u8>> {
    let channels = usize::from(wave_form.channels);
    let bytes = usize::from(wave_form.bits_per_sample / 8);
    if channels == 0 || bytes == 0 || !wave_form.bits_per_sample.is_multiple_of(8) {
        return None;
    }
    let frames = data.chunks_exact(channels * bytes);
    let average = |frame: &[u8], to_f32: &dyn Fn(&[u8]) -> f32| {
        let mut sum = 0.0f32;
        for sample in frame.chunks_exact(bytes) {
            sum += to_f32(sample);
        }
        sum / channels as f32
    };
    let mono = match (wave_form.format_tag, wave_form.bits_per_sample) {
        (1, 16) => frames
            .flat_map(|frame| {
                let sum: i32 = frame
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|s| i32::from(i16::from_le_bytes(*s)))
                    .sum();
                // miniaudio divides the signed sum by the unsigned channel
                // count, so C converts the sum to unsigned first and the
                // result is cut to 16 bits; for two channels that rounds
                // down instead of toward zero
                (((sum as u32) / channels as u32) as i16).to_le_bytes()
            })
            .collect(),
        (1, 8) => frames
            .map(|frame| {
                // ma_pcm_u8_to_f32__reference, ma_pcm_f32_to_u8__reference
                let x = average(frame, &|s| f32::from(s[0]) * U8_TO_F32 - 1.0);
                ((x.clamp(-1.0, 1.0) + 1.0) * 127.5) as u8
            })
            .collect(),
        (1, 24) => frames
            .flat_map(|frame| {
                // ma_pcm_s24_to_f32__reference, ma_pcm_f32_to_s24__reference
                let x = average(frame, &|s| {
                    let v = (u32::from(s[0]) << 8 | u32::from(s[1]) << 16 | u32::from(s[2]) << 24)
                        as i32
                        >> 8;
                    v as f32 * S24_TO_F32
                });
                let r = (x.clamp(-1.0, 1.0) * 8_388_607.0) as i32;
                let [b0, b1, b2, _] = r.to_le_bytes();
                [b0, b1, b2]
            })
            .collect(),
        (1, 32) => frames
            .flat_map(|frame| {
                // ma_pcm_s32_to_f32__reference, ma_pcm_f32_to_s32__reference
                let x = average(frame, &|s| {
                    (f64::from(i32::from_le_bytes([s[0], s[1], s[2], s[3]])) / 2_147_483_648.0)
                        as f32
                });
                ((f64::from(x).clamp(-1.0, 1.0) * 2_147_483_647.0) as i32).to_le_bytes()
            })
            .collect(),
        (3, 32) => frames
            .flat_map(|frame| {
                average(frame, &|s| f32::from_le_bytes([s[0], s[1], s[2], s[3]])).to_le_bytes()
            })
            .collect(),
        _ => return None,
    };
    Some(mono)
}

/// Decodes interleaved PCM WAV samples into the interleaved `i32` the FLAC
/// encoder takes, sign-extending to `i32`. WAV stores 8-bit PCM as
/// unsigned bytes biased by 128, and 16- and 24-bit as signed
/// little-endian, as the format defines.
#[cfg(not(target_family = "wasm"))]
fn pcm_samples(data: &[u8], bits_per_sample: u16) -> Vec<i32> {
    match bits_per_sample {
        8 => data.iter().map(|&b| b as i32 - 128).collect(),
        16 => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c) as i32)
            .collect(),
        24 => data
            .as_chunks::<3>()
            .0
            .iter()
            .map(|c| {
                // sign-extend the 24-bit little-endian value into i32
                let v = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
                (v << 8) >> 8
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Replaces a trailing file extension on a sound path, appends when there
/// is none
#[cfg(not(target_family = "wasm"))]
fn replace_sound_extension(path: &str, ext: &str) -> String {
    match str_path_ext(path) {
        Some(_) => {
            let dot = path.rfind('.').unwrap_or(path.len());
            format!("{}.{ext}", &path[..dot])
        }
        None => format!("{path}.{ext}"),
    }
}

pub(crate) fn write(file_version: &Version, sound: &SoundData, writer: &mut BiffWriter) {
    writer.write_string(&sound.name);
    writer.write_string(&sound.path);
    writer.write_string_empty_zero(&sound.internal_name);

    if is_wav(&sound.path.to_owned()) {
        write_wave_form(writer, &sound.wave_form);
    } else {
        // should we be doing something here?
    }

    writer.write_length_prefixed_data(&sound.data);
    writer.write_u8((&sound.output_target).into());
    if file_version.u32() >= NEW_SOUND_FORMAT_VERSION {
        writer.write_u32(sound.volume);
        writer.write_u32(sound.balance);
        writer.write_u32(sound.fade);
        writer.write_u32(sound.volume);
    }
}

fn read_wave_form(reader: &mut BiffReader<'_>) -> Result<WaveForm, BiffError> {
    let format_tag = reader.get_u16_no_remaining_update()?;
    let channels = reader.get_u16_no_remaining_update()?;
    let samples_per_sec = reader.get_u32_no_remaining_update()?;
    let avg_bytes_per_sec = reader.get_u32_no_remaining_update()?;
    let block_align = reader.get_u16_no_remaining_update()?;
    let bits_per_sample = reader.get_u16_no_remaining_update()?;
    let cb_size = reader.get_u16_no_remaining_update()?;
    Ok(WaveForm {
        format_tag,
        channels,
        samples_per_sec,
        avg_bytes_per_sec,
        block_align,
        bits_per_sample,
        cb_size,
    })
}

fn write_wave_form(writer: &mut BiffWriter, wave_form: &WaveForm) {
    writer.write_u16(wave_form.format_tag);
    writer.write_u16(wave_form.channels);
    writer.write_u32(wave_form.samples_per_sec);
    writer.write_u32(wave_form.avg_bytes_per_sec);
    writer.write_u16(wave_form.block_align);
    writer.write_u16(wave_form.bits_per_sample);
    writer.write_u16(wave_form.cb_size);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::vpx::test_support::latin1_string;
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    fn any_wave_form() -> impl Strategy<Value = WaveForm> {
        (
            any::<u16>(),
            any::<u16>(),
            any::<u32>(),
            any::<u32>(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
        )
            .prop_map(
                |(
                    format_tag,
                    channels,
                    samples_per_sec,
                    avg_bytes_per_sec,
                    block_align,
                    bits_per_sample,
                    cb_size,
                )| WaveForm {
                    format_tag,
                    channels,
                    samples_per_sec,
                    avg_bytes_per_sec,
                    block_align,
                    bits_per_sample,
                    cb_size,
                },
            )
    }

    /// A sound as the stream can hold it: the strings are stored as
    /// Latin-1, and only a wav sound carries a wave form, any other format
    /// reads back the default one.
    fn any_sound_data() -> impl Strategy<Value = SoundData> {
        let wav = (latin1_string(), any_wave_form())
            .prop_map(|(stem, wave_form)| (format!("{stem}.wav"), wave_form));
        let other = latin1_string().prop_map(|stem| (format!("{stem}.mp3"), WaveForm::default()));
        (
            latin1_string(),
            prop_oneof![wav, other],
            proptest::collection::vec(any::<u8>(), 0..64),
            latin1_string(),
            any::<u32>(),
            any::<u32>(),
            any::<u32>(),
            any::<OutputTarget>(),
        )
            .prop_map(
                |(
                    name,
                    (path, wave_form),
                    data,
                    internal_name,
                    fade,
                    volume,
                    balance,
                    output_target,
                )| {
                    SoundData {
                        name,
                        path,
                        wave_form,
                        data,
                        internal_name,
                        fade,
                        volume,
                        balance,
                        output_target,
                    }
                },
            )
    }

    proptest! {
        #[test]
        fn any_sound_round_trips_through_its_stream(sound in any_sound_data()) {
            let version = Version::new(1074);
            let mut writer = BiffWriter::new();
            write(&version, &sound, &mut writer);
            let read = read(&version, &mut BiffReader::new(writer.get_data())).unwrap();
            prop_assert_eq!(sound, read);
        }
    }

    #[test]
    fn test_write_read_biff_wav() {
        let sound: SoundData = SoundData {
            name: "test name".to_string(),
            path: "test path.wav".to_string(),
            data: vec![1, 2, 3, 4],
            wave_form: WaveForm {
                format_tag: 1,
                channels: 2,
                samples_per_sec: 3,
                avg_bytes_per_sec: 4,
                block_align: 5,
                bits_per_sample: 6,
                cb_size: 7,
            },
            internal_name: "test internalname".to_string(),
            fade: 1,
            volume: 2,
            balance: 3,
            output_target: OutputTarget::Table,
        };
        let mut writer = BiffWriter::new();
        write(&Version::new(1074), &sound, &mut writer);
        let sound_read =
            read(&Version::new(1074), &mut BiffReader::new(writer.get_data())).unwrap();
        assert_eq!(sound, sound_read);
    }

    #[test]
    fn test_write_read_biff_other() {
        let sound: SoundData = SoundData {
            name: "test name".to_string(),
            path: "test path.mp3".to_string(),
            // 1MB of data
            data: vec![1, 2, 3, 4],
            wave_form: WaveForm::default(),
            internal_name: "test internalname".to_string(),
            fade: 1,
            volume: 2,
            balance: 3,
            output_target: OutputTarget::Backglass,
        };
        let mut writer = BiffWriter::new();
        write(&Version::new(1083), &sound, &mut writer);
        let sound_read =
            read(&Version::new(1083), &mut BiffReader::new(writer.get_data())).unwrap();
        assert_eq!(sound, sound_read);
    }

    #[test]
    fn test_write_read_sound() {
        let data = vec![4, 3, 2, 1, 0];
        let wave_form = WaveForm::default();
        // this field is always 0
        // wave_form.cb_size = data.len() as u16;
        let sound: SoundData = SoundData {
            name: "test name".to_string(),
            path: "test path.wav".to_string(),
            data,
            wave_form,
            internal_name: "test internalname".to_string(),
            fade: 1,
            volume: 2,
            balance: 3,
            output_target: OutputTarget::Backglass,
        };
        let sound_data = write_sound(&sound);
        let mut sound_read = SoundData {
            name: "test name".to_string(),
            path: "test path.wav".to_string(),
            data: Vec::new(),
            wave_form: WaveForm::default(),
            internal_name: "test internalname".to_string(),
            fade: 1,
            volume: 2,
            balance: 3,
            output_target: OutputTarget::Backglass,
        };
        read_sound(&sound_data, &mut sound_read).unwrap();
        assert_eq!(sound, sound_read);
    }

    /// Sounds written with a WAVEFORMATEX style fmt chunk used to make reading fail
    /// https://github.com/jsm174/vpx-editor/issues/58
    #[test]
    fn test_read_sound_wav_with_fmt_extension_size() {
        // 16 bit mono 22050 Hz, an 18 byte fmt chunk with cbSize 0 and 40000 bytes of data
        let data: Vec<u8> = (0..40000u32).map(|i| i as u8).collect();
        let mut wav: Vec<u8> = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(data.len() as u32 + 38).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&18u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // format_tag PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // channels
        wav.extend_from_slice(&22050u32.to_le_bytes());
        wav.extend_from_slice(&44100u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(&0u16.to_le_bytes()); // cbSize
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);

        let mut sound = SoundData {
            name: "test name".to_string(),
            path: "test path.wav".to_string(),
            data: Vec::new(),
            wave_form: WaveForm::default(),
            internal_name: "".to_string(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: OutputTarget::Table,
        };
        read_sound(&wav, &mut sound).unwrap();
        assert_eq!(sound.wave_form.format_tag, 1);
        assert_eq!(sound.wave_form.channels, 1);
        assert_eq!(sound.wave_form.samples_per_sec, 22050);
        assert_eq!(sound.data, data);
    }

    #[test]
    fn test_read_sound_invalid_wav() {
        let mut sound = SoundData {
            name: "broken".to_string(),
            path: "broken.wav".to_string(),
            data: Vec::new(),
            wave_form: WaveForm::default(),
            internal_name: "".to_string(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: OutputTarget::Table,
        };
        let error = read_sound(b"not a wav file at all", &mut sound).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("broken"), "{error}");
    }

    /// We found a vpx file with sound that had a path "* Backglass Output *"
    /// https://github.com/francisdb/vpin/issues/164
    #[test]
    fn test_ext_issue_no_ext() {
        let sound = SoundData {
            name: "test".to_string(),
            path: "* Backglass Output *".to_string(),
            wave_form: Default::default(),
            data: vec![],
            internal_name: "test".to_string(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: OutputTarget::Table,
        };
        assert_eq!(sound.ext(), "wav".to_string());
        assert!(is_wav(&sound.path));
    }

    #[test]
    fn test_ext_variations() {
        let mut sound = SoundData {
            name: "test".to_string(),
            path: r"c:\foo\test.wav".to_string(),
            wave_form: Default::default(),
            data: vec![],
            internal_name: "test".to_string(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: OutputTarget::Table,
        };
        assert_eq!(sound.ext(), "wav".to_string());
        assert!(is_wav(&sound.path));

        sound.path = "test.WAV".to_string();
        assert_eq!(sound.ext(), "WAV".to_string());
        assert!(is_wav(&sound.path));

        sound.path = "test.mp3".to_string();
        assert_eq!(sound.ext(), "mp3".to_string());
        assert!(!is_wav(&sound.path));
    }

    /// Other tools writing sounds.json might not provide the unused internal_name field.
    /// https://github.com/jsm174/vpx-editor/issues/55
    #[test]
    fn test_json_missing_internal_name() {
        let json = serde_json::json!({
            "name": "test name",
            "path": "test path.ogg",
            "fade": 0,
            "volume": 0,
            "balance": 0,
            "output_target": "table",
        });
        let sound: SoundDataJson = serde_json::from_value(json).unwrap();
        assert_eq!(sound.internal_name, "");
    }

    #[test]
    fn test_str_path_ext() {
        assert_eq!(str_path_ext(r"c:\foo\bar\test.wav"), Some("wav"));
        assert_eq!(str_path_ext("test.mp3"), Some("mp3"));
        assert_eq!(str_path_ext("test"), None);
        assert_eq!(str_path_ext("test."), None);
        assert_eq!(str_path_ext(".test"), None);
        assert_eq!(str_path_ext(r"c:\foo.bar\test.wav"), Some("wav"));
        assert_eq!(str_path_ext("/foo.bar/.test"), None);
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
pub(crate) mod flac_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    /// A PCM WAV sound: raw interleaved samples as the vpx stream holds
    /// them (no RIFF header), with the matching wave form. `samples` are
    /// interleaved per channel. A slowly varying ramp, which flac
    /// compresses well so the conversion is a win.
    pub(crate) fn pcm_wav(
        name: &str,
        channels: u16,
        bits_per_sample: u16,
        frames: usize,
    ) -> SoundData {
        let bytes_per_sample = (bits_per_sample / 8) as usize;
        let mut data = Vec::with_capacity(frames * channels as usize * bytes_per_sample);
        for frame in 0..frames {
            for ch in 0..channels as usize {
                // a value within the depth's signed range, varying slowly
                let v = ((frame as i32 + ch as i32 * 7) % 97) - 48;
                match bits_per_sample {
                    8 => data.push((v + 128) as u8), // wav 8-bit is unsigned
                    16 => data.extend_from_slice(&(v as i16).to_le_bytes()),
                    24 => {
                        let b = v.to_le_bytes();
                        data.extend_from_slice(&b[..3]);
                    }
                    _ => unreachable!(),
                }
            }
        }
        let block_align = channels * bits_per_sample / 8;
        SoundData {
            name: name.to_string(),
            path: format!("C:\\sounds\\{name}.wav"),
            wave_form: WaveForm {
                format_tag: 1,
                channels,
                samples_per_sec: 44100,
                avg_bytes_per_sec: 44100 * block_align as u32,
                block_align,
                bits_per_sample,
                cb_size: 0,
            },
            data,
            internal_name: String::new(),
            fade: 0,
            volume: 0,
            balance: 0,
            output_target: OutputTarget::Table,
        }
    }

    /// Decodes a flac file to interleaved i32 samples with claxon, an
    /// independent decoder, so a pass proves the bytes are spec-correct
    /// flac and not merely what flacenc reads back.
    fn decode_flac(flac: &[u8]) -> TestResult<Vec<i32>> {
        let mut reader = claxon::FlacReader::new(std::io::Cursor::new(flac))?;
        Ok(reader.samples().collect::<Result<Vec<i32>, _>>()?)
    }

    /// The original samples as interleaved i32, the way the encoder saw
    /// them, to compare the decoded flac against
    fn expected_samples(data: &[u8], bits_per_sample: u16) -> Vec<i32> {
        pcm_samples(data, bits_per_sample)
    }

    /// The (min, max) block size from a flac file's STREAMINFO. After the
    /// `fLaC` marker the first metadata block is STREAMINFO: a 4-byte block
    /// header, then the data, which opens with min and max block size as
    /// 16-bit big-endian fields. A decoder-independent read: it is the
    /// header a strict decoder checks, not the audio claxon reads back.
    fn streaminfo_block_sizes(flac: &[u8]) -> (u16, u16) {
        assert!(flac.starts_with(b"fLaC"), "not a flac stream");
        let data = &flac[8..]; // skip "fLaC" + the 4-byte metadata block header
        let min = u16::from_be_bytes([data[0], data[1]]);
        let max = u16::from_be_bytes([data[2], data[3]]);
        (min, max)
    }

    #[test]
    fn pcm_wavs_convert_losslessly_across_depths_and_channels() -> TestResult {
        for bits in [8u16, 16, 24] {
            for channels in [1u16, 2] {
                let original = pcm_wav("fx", channels, bits, 4096);
                let before = expected_samples(&original.data, bits);
                let mut sound = pcm_wav("fx", channels, bits, 4096);

                let outcome = sound.wav_to_flac()?;
                assert_eq!(
                    outcome,
                    Some(Flac::Converted),
                    "{bits}-bit {channels}ch should convert"
                );
                assert_eq!(sound.ext(), "flac");
                assert_eq!(sound.path, "C:\\sounds\\fx.flac");
                assert!(sound.data.starts_with(b"fLaC"));
                assert!(sound.data.len() < original.data.len());
                // the flac decodes back to the exact samples: lossless
                assert_eq!(
                    decode_flac(&sound.data)?,
                    before,
                    "{bits}-bit {channels}ch not lossless"
                );
                // not a wav any more, so no stored header
                assert_eq!(sound.wave_form, WaveForm::default());
            }
        }
        Ok(())
    }

    /// A fixed-block stream whose length is not a block-size multiple ends
    /// in a shorter tail frame. Published flacenc 0.5.1 leaves STREAMINFO
    /// saying min == max == the tail length, which strict decoders (the
    /// miniaudio build vpinball plays through) reject; wav_to_flac re-sets
    /// min = max = max_block_size to fix it. Assert that invariant directly
    /// on the header, since claxon decodes either way and would not catch
    /// it. Over 4096 frames, so the tail frame is genuinely short.
    #[test]
    fn a_tail_frame_leaves_min_and_max_block_size_equal() -> TestResult {
        for frames in [4096 + 1, 4096 + 1000, 8192 + 777] {
            let original = pcm_wav("fx", 2, 16, frames);
            let before = expected_samples(&original.data, 16);
            let mut sound = pcm_wav("fx", 2, 16, frames);

            let outcome = sound.wav_to_flac()?;
            assert_eq!(
                outcome,
                Some(Flac::Converted),
                "{frames} frames should convert"
            );

            let (min, max) = streaminfo_block_sizes(&sound.data);
            assert_eq!(
                min, max,
                "{frames} frames: STREAMINFO min ({min}) != max ({max}), \
                 strict decoders reject the tail frame"
            );
            // still lossless with the corrected header
            assert_eq!(
                decode_flac(&sound.data)?,
                before,
                "{frames} frames not lossless"
            );
        }
        Ok(())
    }

    #[test]
    fn a_non_pcm_wav_is_declined() -> TestResult {
        let mut sound = pcm_wav("adpcm", 1, 16, 64);
        sound.wave_form.format_tag = 2; // ADPCM
        assert_eq!(sound.wav_to_flac()?, None);
        // left untouched
        assert_eq!(sound.ext(), "wav");
        Ok(())
    }

    #[test]
    fn a_non_wav_sound_is_declined() -> TestResult {
        let mut sound = pcm_wav("music", 2, 16, 64);
        sound.path = "C:\\sounds\\music.ogg".to_string();
        assert_eq!(sound.wav_to_flac()?, None);
        assert_eq!(sound.ext(), "ogg");
        Ok(())
    }

    #[test]
    fn flac_that_would_not_shrink_is_left_as_a_wav() -> TestResult {
        // a few random-ish bytes: too short and noisy for flac to beat,
        // so the converter keeps the wav
        let mut sound = pcm_wav("tiny", 1, 16, 8);
        sound.data = vec![0x7f, 0x3a, 0x91, 0x08, 0xe2, 0x55, 0x1c, 0xcc];
        let outcome = sound.wav_to_flac()?;
        assert_eq!(outcome, Some(Flac::NotSmaller));
        assert_eq!(sound.ext(), "wav");
        Ok(())
    }
}

#[cfg(test)]
mod json_error_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn invalid_json_numbers_are_errors_not_panics() {
        for value in [json!(1.5), json!(-1), json!(true), json!(null)] {
            assert!(
                serde_json::from_value::<OutputTarget>(value.clone()).is_err(),
                "{value}"
            );
        }
    }
}

#[cfg(test)]
mod vpinball_wav_tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    fn float_stereo() -> WaveForm {
        WaveForm {
            format_tag: 3,
            channels: 2,
            samples_per_sec: 44100,
            avg_bytes_per_sec: 352800,
            block_align: 8,
            bits_per_sample: 32,
            // a value old files hold, vpinball does not keep it
            cb_size: 24932,
        }
    }

    #[test]
    fn a_vpinball_wav_reads_back_without_cb_size() -> TestResult {
        let samples = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let wav = vpinball_wav_file(&float_stereo(), &samples);
        assert_eq!(wav.len(), 44 + samples.len());
        assert_eq!(&wav[..4], b"RIFF");
        let (wave_form, read) = read_vpinball_wav(&wav)?;
        assert_eq!(
            wave_form,
            WaveForm {
                cb_size: 0,
                ..float_stereo()
            }
        );
        assert_eq!(read, samples);
        Ok(())
    }

    #[test]
    fn a_wav_with_more_chunks_is_read_like_vpinball_reads_it() -> TestResult {
        // vpinball takes everything after the first 44 bytes as samples
        let mut wav = vpinball_wav_file(&float_stereo(), &[9, 9]);
        wav.extend_from_slice(b"LIST\x02\x00\x00\x00ab");
        let (_, samples) = read_vpinball_wav(&wav)?;
        assert_eq!(samples, b"\x09\x09LIST\x02\x00\x00\x00ab".to_vec());
        assert!(read_vpinball_wav(&wav[..40]).is_err());
        Ok(())
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod mono_tests {
    use super::flac_tests::pcm_wav;
    use super::*;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    /// Stereo frames, noise after the extremes, and the mono that
    /// miniaudio 0.11.25, built from vpinball's own copy, decodes them to
    /// the way vpinball decodes a playfield sound, per format tag and bits
    #[rustfmt::skip]
    const MINIAUDIO_DOWNMIX: [(u16, u16, &[u8], &[u8]); 5] = [
            (1, 8, &[255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 94, 63, 49, 145, 233, 3, 232, 20, 219, 118, 3, 179, 64, 112, 221, 162, 253, 115, 44, 159, 216, 161, 54, 50, 93, 112, 88, 217, 197, 112, 232, 157], &[255, 0, 255, 0, 255, 0, 255, 0, 78, 97, 118, 126, 168, 91, 88, 191, 184, 101, 188, 52, 102, 152, 154, 194]),
            (1, 16, &[255, 255, 255, 255, 0, 128, 0, 128, 255, 255, 255, 255, 0, 128, 0, 128, 255, 255, 255, 255, 0, 128, 0, 128, 255, 255, 255, 255, 0, 128, 0, 128, 83, 252, 142, 84, 120, 4, 110, 161, 237, 97, 210, 67, 94, 16, 185, 64, 251, 32, 181, 237, 132, 185, 211, 132, 211, 68, 43, 243, 211, 75, 73, 222, 25, 224, 108, 40, 61, 104, 222, 29, 103, 180, 130, 69, 170, 162, 108, 136, 114, 228, 178, 11, 63, 110, 50, 58, 136, 116, 113, 181, 170, 198, 177, 140], &[255, 255, 0, 128, 255, 255, 0, 128, 255, 255, 0, 128, 255, 255, 0, 128, 112, 40, 243, 210, 223, 82, 139, 40, 88, 7, 43, 159, 255, 27, 14, 21, 66, 4, 13, 67, 244, 252, 139, 149, 18, 248, 56, 84, 252, 20, 173, 169]),
            (1, 24, &[255, 255, 255, 255, 255, 255, 0, 0, 128, 0, 0, 128, 255, 255, 255, 255, 255, 255, 0, 0, 128, 0, 0, 128, 255, 255, 255, 255, 255, 255, 0, 0, 128, 0, 0, 128, 255, 255, 255, 255, 255, 255, 0, 0, 128, 0, 0, 128, 85, 244, 60, 37, 195, 152, 45, 175, 189, 15, 99, 141, 33, 169, 52, 66, 134, 74, 195, 84, 248, 200, 157, 212, 92, 161, 131, 143, 185, 144, 8, 77, 47, 198, 18, 65, 202, 183, 142, 95, 211, 151, 70, 137, 217, 217, 55, 230, 159, 163, 20, 241, 83, 251, 229, 103, 124, 253, 252, 32, 71, 100, 49, 6, 133, 141, 18, 110, 140, 192, 101, 130, 193, 101, 12, 221, 168, 110, 93, 211, 106, 139, 118, 56, 144, 32, 163, 38, 144, 234, 179, 23, 9, 5, 222, 27], &[0, 0, 0, 1, 0, 128, 0, 0, 0, 1, 0, 128, 0, 0, 0, 1, 0, 128, 0, 0, 0, 1, 0, 128, 190, 219, 234, 31, 137, 165, 177, 151, 63, 70, 121, 230, 119, 45, 138, 230, 47, 56, 150, 69, 147, 144, 224, 223, 199, 251, 7, 112, 178, 78, 167, 116, 223, 234, 105, 135, 78, 135, 61, 243, 164, 81, 92, 216, 198, 219, 122, 18]),
            (1, 32, &[255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 128, 0, 0, 0, 128, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 128, 0, 0, 0, 128, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 128, 0, 0, 0, 128, 255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 128, 0, 0, 0, 128, 214, 202, 36, 23, 198, 207, 144, 198, 97, 68, 241, 162, 39, 216, 214, 183, 92, 123, 105, 97, 153, 240, 227, 172, 164, 239, 103, 33, 84, 207, 1, 104, 22, 104, 248, 37, 62, 38, 203, 235, 248, 6, 182, 187, 152, 152, 74, 175, 248, 46, 122, 19, 86, 139, 49, 155, 5, 64, 95, 136, 28, 55, 117, 206, 203, 37, 166, 104, 122, 164, 61, 122, 50, 206, 182, 172, 170, 59, 150, 56, 253, 2, 4, 254, 169, 206, 34, 88, 42, 211, 102, 164, 36, 14, 119, 196, 107, 69, 210, 254, 98, 135, 169, 62, 154, 223, 82, 108, 220, 92, 233, 219, 190, 219, 120, 145, 228, 29, 247, 187, 200, 232, 69, 239, 93, 39, 152, 21], &[0, 0, 0, 0, 1, 0, 0, 128, 0, 0, 0, 0, 1, 0, 0, 128, 0, 0, 0, 0, 1, 0, 0, 128, 0, 0, 0, 0, 1, 0, 0, 128, 65, 205, 218, 238, 1, 14, 100, 173, 255, 181, 38, 7, 127, 223, 180, 68, 31, 199, 225, 8, 1, 80, 128, 181, 65, 221, 85, 215, 129, 59, 106, 171, 255, 228, 113, 113, 225, 132, 166, 242, 191, 104, 19, 43, 129, 240, 110, 180, 127, 230, 189, 30, 255, 29, 30, 36, 1, 253, 183, 166, 15, 8, 111, 2]),
            (3, 32, &[0, 0, 128, 63, 0, 0, 128, 191, 255, 255, 127, 63, 255, 255, 127, 191, 0, 0, 128, 63, 0, 0, 128, 191, 255, 255, 127, 63, 255, 255, 127, 191, 0, 0, 128, 63, 0, 0, 128, 191, 255, 255, 127, 63, 255, 255, 127, 191, 0, 0, 128, 63, 0, 0, 128, 191, 255, 255, 127, 63, 255, 255, 127, 191, 251, 132, 31, 191, 16, 214, 99, 63, 165, 182, 104, 63, 99, 6, 5, 63, 86, 198, 97, 191, 196, 92, 23, 62, 235, 183, 127, 63, 218, 196, 26, 191, 198, 7, 169, 60, 30, 14, 177, 189, 3, 25, 128, 190, 104, 42, 134, 62, 180, 230, 80, 191, 105, 138, 39, 63, 189, 34, 69, 63, 88, 72, 178, 60, 133, 33, 227, 61, 146, 200, 112, 63, 190, 79, 243, 188, 249, 34, 181, 190, 106, 31, 15, 191, 105, 203, 117, 62, 44, 218, 58, 62, 53, 36, 108, 191, 85, 164, 161, 190, 234, 15, 27, 63, 222, 131, 193, 61, 54, 44, 115, 191, 137, 60, 52, 191, 228, 212, 85, 191, 223, 8, 111, 63, 58, 100, 57, 190], &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 42, 162, 8, 62, 132, 222, 54, 63, 37, 239, 187, 190, 34, 230, 73, 62, 44, 204, 6, 189, 160, 44, 194, 59, 44, 113, 165, 189, 0, 181, 202, 62, 97, 150, 6, 63, 245, 87, 68, 190, 32, 89, 35, 190, 170, 109, 189, 190, 127, 123, 20, 62, 186, 251, 218, 190, 182, 8, 69, 191, 208, 175, 192, 62]),
    ];

    #[test]
    fn the_downmix_is_miniaudios() {
        for (format_tag, bits, stereo, mono) in MINIAUDIO_DOWNMIX {
            let wave_form = WaveForm {
                format_tag,
                channels: 2,
                samples_per_sec: 44100,
                avg_bytes_per_sec: 44100 * u32::from(bits / 4),
                block_align: bits / 4,
                bits_per_sample: bits,
                cb_size: 0,
            };
            assert_eq!(
                downmix_wav(stereo, &wave_form).as_deref(),
                Some(mono),
                "format {format_tag} at {bits} bits"
            );
        }
    }

    #[test]
    fn a_stereo_wav_becomes_a_mono_wav_of_the_same_format() -> TestResult {
        let mut sound = pcm_wav("hit", 2, 24, 1000);
        assert_eq!(sound.downmix_to_mono()?, Mono::Converted);
        assert_eq!(sound.data.len(), 3000);
        assert_eq!(
            (
                sound.wave_form.channels,
                sound.wave_form.block_align,
                sound.wave_form.avg_bytes_per_sec,
                sound.wave_form.bits_per_sample
            ),
            (1, 3, 44100 * 3, 24)
        );
        Ok(())
    }

    #[test]
    fn a_stereo_flac_becomes_a_mono_flac_one_bit_deeper() -> TestResult {
        let mut sound = pcm_wav("hit", 2, 16, 20_000);
        let stereo = pcm_samples(&sound.data, 16);
        assert_eq!(sound.wav_to_flac()?, Some(Flac::Converted));

        assert_eq!(sound.downmix_to_mono()?, Mono::Converted);

        // claxon does not read a depth the frame headers cannot name
        use flacenc::component::Decode;
        let (_, stream) =
            flacenc::component::parser::stream::<nom::error::Error<&[u8]>>(&sound.data)
                .map_err(|e| e.to_string())?;
        assert_eq!(
            (
                stream.stream_info().channels(),
                stream.stream_info().bits_per_sample()
            ),
            (1, 17)
        );
        let mono: Vec<i32> = (0..stream.frame_count())
            .filter_map(|i| stream.frame(i))
            .flat_map(|frame| frame.decode())
            .collect();
        let sums: Vec<i32> = stereo
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[l, r]| l + r)
            .collect();
        assert_eq!(mono, sums);
        Ok(())
    }

    #[test]
    fn lossy_and_deep_files_are_left_alone() -> TestResult {
        let mut mp3 = pcm_wav("music", 2, 16, 10);
        mp3.path = "music.mp3".to_string();
        mp3.data = vec![0xFF, 0xFB, 0x90, 0x64, 0, 0];
        assert_eq!(mp3.downmix_to_mono()?, Mono::Lossy);

        let mut deep = pcm_wav("deep", 2, 24, 2000);
        assert_eq!(deep.wav_to_flac()?, Some(Flac::Converted));
        assert_eq!(deep.downmix_to_mono()?, Mono::TooDeep);

        let mut adpcm = pcm_wav("adpcm", 2, 16, 10);
        adpcm.wave_form.format_tag = 2;
        assert_eq!(adpcm.downmix_to_mono()?, Mono::NotPcm);
        Ok(())
    }
}
