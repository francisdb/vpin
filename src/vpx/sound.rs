use std::fmt;

use bytes::{Buf, BufMut, BytesMut};
use serde::{Deserialize, Serialize};

use super::{
    biff::{BiffReader, BiffWriter},
    Version,
};

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

#[derive(PartialEq)]
pub struct SoundData {
    pub name: String,
    pub path: String,
    pub wave_form: WaveForm,
    pub data: Vec<u8>,
    /// Removed: previously did write the same name again, but just in lower case
    /// This rudimentary version here needs to stay as otherwise problems when loading, as one field less
    /// Now just writes a short dummy/empty string.
    /// see https://github.com/vpinball/vpinball/commit/3320dd11d66ecedba326197c7d4e85c48864cc19
    pub internal_name: String,
    pub fade: u32,
    pub volume: u32,
    pub balance: u32,
    pub output_target: u8,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct SoundDataJson {
    name: String,
    path: String,
    internal_name: String,
    fade: u32,
    volume: u32,
    balance: u32,
    output_target: u8,
    // in case we have a duplicate name
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
            output_target: sound_data.output_target,
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
            output_target: self.output_target,
        }
    }
}

const WAV_HEADER_SIZE: usize = 44;

fn write_wav_header2(sound_data: &SoundData) -> Vec<u8> {
    let mut buf = BytesMut::with_capacity(WAV_HEADER_SIZE);
    buf.put(&b"RIFF"[..]); // 4
    buf.put_u32_le(sound_data.data.len() as u32 + 36); // 4
    buf.put(&b"WAVE"[..]); // 4
    buf.put(&b"fmt "[..]); // 4
    buf.put_u32_le(16); // 4
    buf.put_u16_le(sound_data.wave_form.format_tag); // 2
    buf.put_u16_le(sound_data.wave_form.channels); // 2
    buf.put_u32_le(sound_data.wave_form.samples_per_sec); // 4
    buf.put_u32_le(
        sound_data.wave_form.samples_per_sec
            * sound_data.wave_form.bits_per_sample as u32
            * sound_data.wave_form.channels as u32
            / 8,
    ); // 4
    buf.put_u16_le(sound_data.wave_form.block_align); // 2
    buf.put_u16_le(sound_data.wave_form.bits_per_sample); // 2
    buf.put(&b"data"[..]); // 4
    let data_len = if sound_data.wave_form.format_tag == 1 {
        // In the vpx file for PCM this is always 0,
        // so we can use the length of the data.
        sound_data.data.len() as u32 // 4
    } else {
        sound_data.wave_form.cb_size as u32 // 4
    };
    buf.put_u32_le(data_len); // 4
    buf.to_vec() // total 44 bytes
}

#[derive(Debug, PartialEq)]
struct WavHeader {
    size: u32,
    fmt_size: u32,
    format_tag: u16,
    channels: u16,
    samples_per_sec: u32,
    avg_bytes_per_sec: u32,
    block_align: u16,
    bits_per_sample: u16,
    // These fields are only present if format tag is not 1: PCM
    extension_size: Option<u16>,
    extra_fields: Vec<u8>,
    data_size: u32,
}

impl Default for WavHeader {
    fn default() -> Self {
        // These are some common values for the format_tag
        // 1: PCM (Pulse Code Modulation) - Uncompressed data
        // 2: Microsoft ADPCM
        // 3: IEEE Float
        // 6: 8-bit ITU-T G.711 A-law
        // 7: 8-bit ITU-T G.711 µ-law
        // 17: IMA ADPCM
        // 20: ITU-T G.723 ADPCM (Yamaha)
        // 49: GSM 6.10
        // 64: ITU-T G.721 ADPCM
        // 80: MPEG
        // 65534: Experimental

        // Typical 2-channel, 16-bit PCM WAV header
        // format_tag: 1 (PCM)
        // channels: 2 (stereo)
        // samples_per_sec: 44100 (standard CD quality)
        // avg_bytes_per_sec: 176400 (44100 samples/sec * 2 channels * 2 bytes/sample)
        // block_align: 4 (2 channels * 2 bytes/sample)
        // bits_per_sample: 16 (standard CD quality)
        WavHeader {
            size: 0,
            fmt_size: 16,
            format_tag: 1,
            channels: 2,
            samples_per_sec: 44100,
            avg_bytes_per_sec: 176400,
            block_align: 4,
            bits_per_sample: 16,
            extension_size: None,
            extra_fields: Vec::new(),
            data_size: 0,
        }
    }
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

fn write_wav_header(wav_header: &WavHeader, writer: &mut BytesMut) {
    writer.put(&b"RIFF"[..]);
    writer.put_u32_le(wav_header.size);
    writer.put(&b"WAVE"[..]);
    writer.put(&b"fmt "[..]);
    writer.put_u32_le(wav_header.fmt_size);
    writer.put_u16_le(wav_header.format_tag);
    writer.put_u16_le(wav_header.channels);
    writer.put_u32_le(wav_header.samples_per_sec);
    writer.put_u32_le(wav_header.avg_bytes_per_sec);
    writer.put_u16_le(wav_header.block_align);
    writer.put_u16_le(wav_header.bits_per_sample);
    if let Some(extension_size) = wav_header.extension_size {
        writer.put_u16_le(extension_size);
        writer.put(&wav_header.extra_fields[..]);
    }
    writer.put(&b"data"[..]);
    writer.put_u32_le(wav_header.data_size);
}

fn read_wav_header(reader: &mut BytesMut) -> WavHeader {
    reader.expect_bytes(b"RIFF");
    let size = reader.get_u32_le();
    reader.expect_bytes(b"WAVE");
    reader.expect_bytes(b"fmt ");
    let fmt_size = reader.get_u32_le();
    let format_tag = reader.get_u16_le();
    let channels = reader.get_u16_le();
    let samples_per_sec = reader.get_u32_le();
    let avg_bytes_per_sec = reader.get_u32_le();
    let block_align = reader.get_u16_le();
    let bits_per_sample = reader.get_u16_le();
    let (extension_size, extra_fields) = match format_tag {
        1 => (None, Vec::<u8>::new()),
        3 => (Some(0), Vec::<u8>::new()),
        _ => {
            panic!("unsupported format_tag: {}", format_tag);
            // let extension_size = reader.get_u16_le();
            // let extra_fields = reader.read_bytes_vec(extension_size as usize);
            // (Some(extension_size), extra_fields)
        }
    };

    reader.expect_bytes(b"data");
    let data_size = reader.get_u32_le();
    WavHeader {
        size,
        fmt_size,
        format_tag,
        channels,
        samples_per_sec,
        avg_bytes_per_sec,
        block_align,
        bits_per_sample,
        extension_size,
        extra_fields: Vec::new(),
        data_size,
    }
}

trait ReadBytesExt {
    fn read_bytes_vec(&mut self, n: usize) -> Vec<u8>;
    fn read_bytes<const N: usize>(&mut self) -> [u8; N];
    fn expect_bytes<const N: usize>(&mut self, expected: &[u8; N]);
}

impl ReadBytesExt for BytesMut {
    fn read_bytes_vec(&mut self, n: usize) -> Vec<u8> {
        let mut arr = Vec::with_capacity(n);
        self.copy_to_slice(&mut arr);
        arr
    }

    fn read_bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut arr = [0; N];
        self.copy_to_slice(&mut arr);
        arr
    }

    fn expect_bytes<const N: usize>(&mut self, expected: &[u8; N]) {
        let bytes = self.read_bytes();
        assert_eq!(&bytes, expected);
    }
}

pub fn write_sound(sound_data: &SoundData) -> Vec<u8> {
    if is_wav(&sound_data.path) {
        if sound_data.wave_form.format_tag != 1 {
            println!(
                "write_sound: {} {:?}",
                sound_data.path, sound_data.wave_form
            );
        }
        let mut buf = BytesMut::with_capacity(WAV_HEADER_SIZE + sound_data.data.len());
        buf.put_slice(&write_wav_header2(sound_data));
        buf.put_slice(&sound_data.data);
        buf.to_vec()
    } else {
        sound_data.data.clone()
    }
}

pub fn read_sound(data: &[u8], sound_data: &mut SoundData) {
    if is_wav(&sound_data.path) {
        let mut reader = bytes::BytesMut::from(data);
        let header = read_wav_header(&mut reader);
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
}

#[derive(Debug, PartialEq)]
pub struct WaveForm {
    // Format type
    pub format_tag: u16,
    // Number of channels (i.e. mono, stereo...)
    pub channels: u16,
    // Sample rate
    pub samples_per_sec: u32,
    // For buffer estimation
    pub avg_bytes_per_sec: u32,
    // Block size of data
    pub block_align: u16,
    // Number of bits per sample of mono data
    pub bits_per_sample: u16,
    // The count in bytes of the size of extra information (after cbSize)
    // Seems to always be 0 in the vpx file if the format_tag is 1
    pub cb_size: u16,
}

impl WaveForm {
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
        // TODO we might want to also check the jpeg fsPath
        match self.path.split('.').last() {
            Some(ext) => ext.to_string(),
            None => "bin".to_string(),
        }
    }
}

pub(crate) fn read(file_version: &Version, reader: &mut BiffReader) -> SoundData {
    let mut name: String = "".to_string();
    let mut path: String = "".to_string();
    let mut internal_name: String = "".to_string();
    let mut fade: u32 = 0;
    let mut volume: u32 = 0;
    let mut balance: u32 = 0;
    let mut output_target: u8 = 0;
    let mut data: Vec<u8> = Vec::new();
    let mut wave_form: WaveForm = WaveForm::new();

    // TODO add support for the old format file version < 1031
    // https://github.com/freezy/VisualPinball.Engine/blob/ec1e9765cd4832c134e889d6e6d03320bc404bd5/VisualPinball.Engine/VPT/Sound/SoundData.cs#L98

    let num_values = if file_version.u32() < NEW_SOUND_FORMAT_VERSION {
        6
    } else {
        10
    };

    for i in 0..num_values {
        match i {
            0 => {
                name = reader.get_string_no_remaining_update();
            }
            1 => {
                path = reader.get_string_no_remaining_update();
            }
            2 => {
                internal_name = reader.get_string_no_remaining_update();
            }
            3 => {
                if is_wav(&path.to_owned()) {
                    wave_form = read_wave_form(reader);
                } else {
                    // should we be doing something here?
                }
            }
            4 => {
                data = reader.get_data_no_remaining_update();
            }
            5 => {
                output_target = reader.get_u8_no_remaining_update();
            }
            6 => {
                volume = reader.get_u32_no_remaining_update();
            }
            7 => {
                balance = reader.get_u32_no_remaining_update();
            }
            8 => {
                fade = reader.get_u32_no_remaining_update();
            }
            9 => {
                // TODO why do we have the volume twice?
                volume = reader.get_u32_no_remaining_update();
            }
            unexpected => {
                panic!("unexpected value {}", unexpected);
            }
        }
    }

    SoundData {
        name,
        path,
        data: data.to_vec(),
        wave_form,
        internal_name,
        fade,
        volume,
        balance,
        output_target,
    }
}

fn is_wav(path: &str) -> bool {
    path.to_lowercase().ends_with(".wav")
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
    writer.write_u8(sound.output_target);
    if file_version.u32() >= NEW_SOUND_FORMAT_VERSION {
        writer.write_u32(sound.volume);
        writer.write_u32(sound.balance);
        writer.write_u32(sound.fade);
        writer.write_u32(sound.volume);
    }
}

fn read_wave_form(reader: &mut BiffReader<'_>) -> WaveForm {
    let format_tag = reader.get_u16_no_remaining_update();
    let channels = reader.get_u16_no_remaining_update();
    let samples_per_sec = reader.get_u32_no_remaining_update();
    let avg_bytes_per_sec = reader.get_u32_no_remaining_update();
    let block_align = reader.get_u16_no_remaining_update();
    let bits_per_sample = reader.get_u16_no_remaining_update();
    let cb_size = reader.get_u16_no_remaining_update();
    let wave_form = WaveForm {
        format_tag,
        channels,
        samples_per_sec,
        avg_bytes_per_sec,
        block_align,
        bits_per_sample,
        cb_size,
    };
    if wave_form.format_tag != 1 {
        println!("read wave_form: {:?}", wave_form);
    }
    wave_form
}

fn write_wave_form(writer: &mut BiffWriter, wave_form: &WaveForm) {
    if wave_form.format_tag != 1 {
        println!("write wave_form: {:?}", wave_form);
    }
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
    use pretty_assertions::assert_eq;

    // TODO add test for non-wav sound

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
            output_target: 4,
        };
        let mut writer = BiffWriter::new();
        write(&Version::new(1074), &sound, &mut writer);
        let sound_read = read(&Version::new(1074), &mut BiffReader::new(writer.get_data()));
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
            output_target: 4,
        };
        let mut writer = BiffWriter::new();
        write(&Version::new(1083), &sound, &mut writer);
        let sound_read = read(&Version::new(1083), &mut BiffReader::new(writer.get_data()));
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
            output_target: 4,
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
            output_target: 4,
        };
        read_sound(&sound_data, &mut sound_read);
        assert_eq!(sound, sound_read);
    }

    #[test]
    fn test_write_read_wav_header() {
        let header = WavHeader {
            size: 120 + 36,
            fmt_size: 16,
            format_tag: 1,
            channels: 1,
            samples_per_sec: 44100,
            avg_bytes_per_sec: 88200,
            block_align: 2,
            bits_per_sample: 16,
            extension_size: None,
            extra_fields: Vec::new(),
            data_size: 120,
        };
        let mut writer = BytesMut::new();
        write_wav_header(&header, &mut writer);
        let mut reader = BytesMut::from(writer);
        let header_read = read_wav_header(&mut reader);
        assert_eq!(header, header_read);
    }

    fn test_write_read_wav_header_pcm_float() {
        let header = WavHeader {
            size: 120 + 36,
            fmt_size: 16,
            format_tag: 3,
            channels: 1,
            samples_per_sec: 44100,
            avg_bytes_per_sec: 88200,
            block_align: 2,
            bits_per_sample: 16,
            extension_size: Some(0),
            extra_fields: vec![],
            data_size: 120,
        };
        let mut writer = BytesMut::new();
        write_wav_header(&header, &mut writer);
        let mut reader = BytesMut::from(writer);
        let header_read = read_wav_header(&mut reader);
        assert_eq!(header, header_read);
    }
}
