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

impl SoundData {
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
fn is_wav(path: &str) -> bool {
    match str_path_ext(path) {
        Some(ext) => ext.eq_ignore_ascii_case("wav"),
        None => true,
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
