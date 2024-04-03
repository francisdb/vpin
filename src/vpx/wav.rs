use bytes::{Buf, BufMut, BytesMut};

// TODO replace with a library that can read and write wav file headers
//   one option could be "hound"

// An example of a float format wav file can be found in
// FirePower II (Williams 1983) 1.1.vpx Ding_01.wav

#[derive(Debug, PartialEq)]
pub(crate) struct WavHeader {
    pub(crate) size: u32,
    pub(crate) fmt_size: u32,
    pub(crate) format_tag: u16,
    pub(crate) channels: u16,
    pub(crate) samples_per_sec: u32,
    pub(crate) avg_bytes_per_sec: u32,
    pub(crate) block_align: u16,
    pub(crate) bits_per_sample: u16,
    // These fields are only present if format tag is not 1: PCM
    pub(crate) extension_size: Option<u16>,
    pub(crate) extra_fields: Vec<u8>,
    pub(crate) data_size: u32,
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

pub(crate) fn write_wav_header(wav_header: &WavHeader, writer: &mut BytesMut) {
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
    if wav_header.format_tag != 1 && wav_header.extension_size.is_none() {
        panic!(
            "format_tag {} requires extension_size",
            wav_header.format_tag
        );
    }
    if let Some(extension_size) = wav_header.extension_size {
        writer.put_u16_le(extension_size);
        writer.put(&wav_header.extra_fields[..]);
    }
    writer.put(&b"data"[..]);
    writer.put_u32_le(wav_header.data_size);
}

pub(crate) fn read_wav_header(reader: &mut BytesMut) -> WavHeader {
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
    let (extension_size, _extra_fields) = match format_tag {
        1 => (None, Vec::<u8>::new()),
        3 => {
            let extension_size = reader.get_u16_le();
            let extra_fields = reader.read_bytes_vec(extension_size as usize);
            (Some(extension_size), extra_fields)
        }
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

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

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
        let mut bytes_mut = BytesMut::new();
        write_wav_header(&header, &mut bytes_mut);
        let header_read = read_wav_header(&mut bytes_mut);
        assert_eq!(header, header_read);
    }

    #[test]
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
        let mut bytes_mut = BytesMut::new();
        write_wav_header(&header, &mut bytes_mut);
        let header_read = read_wav_header(&mut bytes_mut);
        assert_eq!(header, header_read);
    }
}
