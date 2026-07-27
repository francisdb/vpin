use bytes::{Buf, BufMut, BytesMut};
use std::io;

// This parses the wav container only, it never touches samples. Sample level crates like
// "hound" are not a fit: vpinball stores a raw WAVEFORMATEX in the vpx, so we have to keep
// format_tag, block_align, avg_bytes_per_sec and cbSize, and pass through the chunks and the
// data blob verbatim. Those crates normalize all of that away and reject the formats they
// can not decode.

// An example of a float format wav file can be found in
// FirePower II (Williams 1983) 1.1.vpx Ding_01.wav

/// Size of the fmt chunk for the plain WAVEFORMAT/PCMWAVEFORMAT layout
const FMT_SIZE_MIN: u32 = 16;
/// Size of the fmt chunk for a WAVEFORMATEX layout without extra bytes
const FMT_SIZE_EXTENSIBLE: u32 = 18;

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
    /// The cbSize field, only present for a WAVEFORMATEX style fmt chunk
    pub(crate) extension_size: Option<u16>,
    /// The bytes of the fmt chunk that follow cbSize
    pub(crate) extension_fields: Vec<u8>,
    /// Chunks before the fmt chunk (e.g. "JUNK" or "bext"), kept verbatim
    pub(crate) pre_fmt_fields: Vec<u8>,
    /// Chunks between the fmt chunk and the data chunk (e.g. "fact" or "LIST"), kept verbatim
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
            fmt_size: FMT_SIZE_MIN,
            format_tag: 1,
            channels: 2,
            samples_per_sec: 44100,
            avg_bytes_per_sec: 176400,
            block_align: 4,
            bits_per_sample: 16,
            extension_size: None,
            extension_fields: Vec::new(),
            pre_fmt_fields: Vec::new(),
            extra_fields: Vec::new(),
            data_size: 0,
        }
    }
}

pub(crate) fn write_wav_header(wav_header: &WavHeader, writer: &mut BytesMut) {
    writer.put(&b"RIFF"[..]);
    writer.put_u32_le(wav_header.size);
    writer.put(&b"WAVE"[..]);
    // write chunks that came before the fmt chunk (e.g. "JUNK" or "bext")
    writer.put(&wav_header.pre_fmt_fields[..]);
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
        writer.put(&wav_header.extension_fields[..]);
    }
    if wav_header.fmt_size % 2 == 1 {
        // RIFF chunks are word aligned, an odd sized chunk is followed by a pad byte
        writer.put_u8(0);
    }
    // write extra chunks between fmt and data (e.g. "fact" chunk)
    writer.put(&wav_header.extra_fields[..]);
    writer.put(&b"data"[..]);
    writer.put_u32_le(wav_header.data_size);
}

/// The parts of a WavHeader that come from the fmt chunk
struct FmtChunk {
    fmt_size: u32,
    format_tag: u16,
    channels: u16,
    samples_per_sec: u32,
    avg_bytes_per_sec: u32,
    block_align: u16,
    bits_per_sample: u16,
    extension_size: Option<u16>,
    extension_fields: Vec<u8>,
}

/// Walks the RIFF chunks until the data chunk, picking out the fmt chunk on the way.
/// Chunks we do not know are kept verbatim so we can write them back in the same order.
/// The fmt chunk is not required to come first, files with a "JUNK" or "bext" chunk in
/// front of it are valid.
pub(crate) fn read_wav_header(reader: &mut BytesMut) -> io::Result<WavHeader> {
    reader.expect_bytes(b"RIFF")?;
    let size = reader.read_u32_le()?;
    reader.expect_bytes(b"WAVE")?;

    let mut fmt: Option<FmtChunk> = None;
    let mut pre_fmt_fields: Vec<u8> = Vec::new();
    let mut extra_fields: Vec<u8> = Vec::new();

    loop {
        let chunk_id: [u8; 4] = reader.read_bytes()?;
        if !is_chunk_id(&chunk_id) {
            return Err(invalid_data(format!(
                "unexpected wav chunk id {:?} while looking for the data chunk",
                String::from_utf8_lossy(&chunk_id)
            )));
        }
        let chunk_size = reader.read_u32_le()?;

        if chunk_id == *b"data" {
            let fmt = fmt.ok_or_else(|| {
                invalid_data("wav has no fmt chunk before the data chunk".to_string())
            })?;
            return Ok(WavHeader {
                size,
                fmt_size: fmt.fmt_size,
                format_tag: fmt.format_tag,
                channels: fmt.channels,
                samples_per_sec: fmt.samples_per_sec,
                avg_bytes_per_sec: fmt.avg_bytes_per_sec,
                block_align: fmt.block_align,
                bits_per_sample: fmt.bits_per_sample,
                extension_size: fmt.extension_size,
                extension_fields: fmt.extension_fields,
                pre_fmt_fields,
                extra_fields,
                data_size: chunk_size,
            });
        }

        if chunk_id == *b"fmt " {
            if fmt.is_some() {
                return Err(invalid_data("wav has more than one fmt chunk".to_string()));
            }
            fmt = Some(read_fmt_chunk(reader, chunk_size)?);
            continue;
        }

        // RIFF chunks are word aligned, an odd sized chunk is followed by a pad byte.
        // Compute in u64, this overflows usize on 32 bit targets for a bogus size.
        let padded_size = (chunk_size as u64).next_multiple_of(2);
        // keep the chunk verbatim, including the pad byte, so we can write it back as is
        let payload = reader.read_bytes_vec(padded_size)?;
        let target = if fmt.is_some() {
            &mut extra_fields
        } else {
            &mut pre_fmt_fields
        };
        target.extend_from_slice(&chunk_id);
        target.extend_from_slice(&chunk_size.to_le_bytes());
        target.extend_from_slice(&payload);
    }
}

fn read_fmt_chunk(reader: &mut BytesMut, fmt_size: u32) -> io::Result<FmtChunk> {
    if fmt_size != FMT_SIZE_MIN && fmt_size < FMT_SIZE_EXTENSIBLE {
        return Err(invalid_data(format!(
            "wav fmt chunk is {fmt_size} bytes, expected {FMT_SIZE_MIN} or at least {FMT_SIZE_EXTENSIBLE}"
        )));
    }
    let mut chunk =
        BytesMut::from(&reader.read_bytes_vec((fmt_size as u64).next_multiple_of(2))?[..]);
    let format_tag = chunk.read_u16_le()?;
    let channels = chunk.read_u16_le()?;
    let samples_per_sec = chunk.read_u32_le()?;
    let avg_bytes_per_sec = chunk.read_u32_le()?;
    let block_align = chunk.read_u16_le()?;
    let bits_per_sample = chunk.read_u16_le()?;

    // Whether the cbSize extension field is present is decided by the size of the fmt chunk,
    // not by the format tag. Both a 16 byte (PCMWAVEFORMAT) and an 18 byte (WAVEFORMATEX)
    // fmt chunk are seen in the wild for PCM (format_tag 1).
    // See https://github.com/jsm174/vpx-editor/issues/58
    let (extension_size, extension_fields) = if fmt_size >= FMT_SIZE_EXTENSIBLE {
        let extension_size = chunk.read_u16_le()?;
        // The fmt chunk size is authoritative, cbSize is not always consistent with it.
        let extension_fields = chunk.read_bytes_vec((fmt_size - FMT_SIZE_EXTENSIBLE) as u64)?;
        (Some(extension_size), extension_fields)
    } else if reader.looks_like_stray_extension_size() {
        // vpin up to 0.26.x wrote a cbSize field for non-PCM formats while still reporting a
        // fmt chunk size of 16. Recover from those files instead of failing on them.
        (Some(reader.read_u16_le()?), Vec::new())
    } else {
        (None, Vec::new())
    };

    Ok(FmtChunk {
        fmt_size,
        format_tag,
        channels,
        samples_per_sec,
        avg_bytes_per_sec,
        block_align,
        bits_per_sample,
        extension_size,
        extension_fields,
    })
}

/// RIFF chunk ids are four printable ascii characters
fn is_chunk_id(bytes: &[u8; 4]) -> bool {
    bytes.iter().all(|b| b.is_ascii_graphic() || *b == b' ')
}

fn invalid_data(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

trait ReadBytesExt {
    fn read_bytes_vec(&mut self, n: u64) -> io::Result<Vec<u8>>;
    fn read_bytes<const N: usize>(&mut self) -> io::Result<[u8; N]>;
    fn read_u16_le(&mut self) -> io::Result<u16>;
    fn read_u32_le(&mut self) -> io::Result<u32>;
    fn expect_bytes<const N: usize>(&mut self, expected: &[u8; N]) -> io::Result<()>;
    fn looks_like_stray_extension_size(&self) -> bool;
}

impl ReadBytesExt for BytesMut {
    /// Reads `n` bytes, `n` is a u64 as a malformed file can declare a chunk size that does
    /// not fit a usize on 32 bit targets like wasm32. Never allocates more than what is left
    /// in the buffer, so a bogus chunk size can not blow up the allocator.
    fn read_bytes_vec(&mut self, n: u64) -> io::Result<Vec<u8>> {
        if n > self.remaining() as u64 {
            return Err(invalid_data(format!(
                "unexpected end of wav data, wanted {n} bytes but only {} left",
                self.remaining()
            )));
        }
        Ok(self.split_to(n as usize).to_vec())
    }

    fn read_bytes<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut arr = [0; N];
        if self.remaining() < N {
            return Err(invalid_data(format!(
                "unexpected end of wav data, wanted {N} bytes but only {} left",
                self.remaining()
            )));
        }
        self.copy_to_slice(&mut arr);
        Ok(arr)
    }

    fn read_u16_le(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.read_bytes()?))
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.read_bytes()?))
    }

    fn expect_bytes<const N: usize>(&mut self, expected: &[u8; N]) -> io::Result<()> {
        let bytes: [u8; N] = self.read_bytes()?;
        if &bytes != expected {
            return Err(invalid_data(format!(
                "expected {:?} in wav data but found {:?}",
                String::from_utf8_lossy(expected),
                String::from_utf8_lossy(&bytes)
            )));
        }
        Ok(())
    }

    /// True if the next two bytes are not the start of a chunk id but the two bytes after
    /// them are, which means a cbSize field is present that the fmt chunk size did not
    /// account for.
    fn looks_like_stray_extension_size(&self) -> bool {
        if self.remaining() < 6 {
            return false;
        }
        let head: [u8; 4] = self[0..4].try_into().unwrap();
        let shifted: [u8; 4] = self[2..6].try_into().unwrap();
        !is_chunk_id(&head) && is_chunk_id(&shifted)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use nom::AsBytes;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_read_write_wav_header() {
        let data = include_bytes!("../../testdata/fx_coin_converted.wav");
        let mut bytes_mut_in = BytesMut::from(data.as_bytes());
        let header_read = read_wav_header(&mut bytes_mut_in).unwrap();
        let mut bytes_mut_out = BytesMut::new();
        write_wav_header(&header_read, &mut bytes_mut_out);
        assert_eq!(data[..78], bytes_mut_out[..78]);
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
            extension_fields: Vec::new(),
            pre_fmt_fields: Vec::new(),
            extra_fields: Vec::new(),
            data_size: 120,
        };
        let mut bytes_mut = BytesMut::new();
        write_wav_header(&header, &mut bytes_mut);
        let header_read = read_wav_header(&mut bytes_mut).unwrap();
        assert_eq!(header, header_read);
    }

    // https://github.com/francisdb/vpin/issues/102
    #[test]
    fn test_write_read_wav_header_pcm_float() {
        let header = WavHeader {
            size: 120 + 36,
            fmt_size: 18,
            format_tag: 3,
            channels: 1,
            samples_per_sec: 44100,
            avg_bytes_per_sec: 88200,
            block_align: 2,
            bits_per_sample: 16,
            extension_size: Some(0),
            extension_fields: Vec::new(),
            pre_fmt_fields: Vec::new(),
            extra_fields: Vec::new(),
            data_size: 120,
        };
        let mut bytes_mut = BytesMut::new();
        write_wav_header(&header, &mut bytes_mut);
        let header_read = read_wav_header(&mut bytes_mut).unwrap();
        assert_eq!(header, header_read);
    }

    /// A PCM file with a WAVEFORMATEX (18 byte) fmt chunk, as written by several tools.
    /// https://github.com/jsm174/vpx-editor/issues/58
    #[test]
    fn test_read_wav_header_pcm_with_extension_size() {
        let header = WavHeader {
            size: 40000 + 38,
            fmt_size: 18,
            format_tag: 1,
            channels: 1,
            samples_per_sec: 22050,
            avg_bytes_per_sec: 44100,
            block_align: 2,
            bits_per_sample: 16,
            extension_size: Some(0),
            extension_fields: Vec::new(),
            pre_fmt_fields: Vec::new(),
            extra_fields: Vec::new(),
            // low 16 bits >= 0x8000, which used to be read as a bogus chunk size
            data_size: 40000,
        };
        let mut bytes_mut = BytesMut::new();
        write_wav_header(&header, &mut bytes_mut);
        let header_read = read_wav_header(&mut bytes_mut).unwrap();
        assert_eq!(header, header_read);
    }

    /// A fmt chunk with extra bytes after cbSize, e.g. WAVE_FORMAT_EXTENSIBLE
    #[test]
    fn test_read_wav_header_fmt_extension_fields() {
        let header = WavHeader {
            size: 120 + 60,
            fmt_size: 40,
            format_tag: 0xFFFE,
            channels: 1,
            samples_per_sec: 22050,
            avg_bytes_per_sec: 44100,
            block_align: 2,
            bits_per_sample: 16,
            extension_size: Some(22),
            extension_fields: (0u8..22).collect(),
            pre_fmt_fields: Vec::new(),
            extra_fields: Vec::new(),
            data_size: 120,
        };
        let mut bytes_mut = BytesMut::new();
        write_wav_header(&header, &mut bytes_mut);
        let header_read = read_wav_header(&mut bytes_mut).unwrap();
        assert_eq!(header, header_read);
    }

    /// vpin up to 0.26.x wrote a cbSize field while reporting a fmt chunk size of 16
    #[test]
    fn test_read_wav_header_legacy_vpin_extension_size() {
        let mut data = BytesMut::new();
        data.put(&b"RIFF"[..]);
        data.put_u32_le(156);
        data.put(&b"WAVE"[..]);
        data.put(&b"fmt "[..]);
        data.put_u32_le(16);
        data.put_u16_le(3);
        data.put_u16_le(1);
        data.put_u32_le(44100);
        data.put_u32_le(88200);
        data.put_u16_le(2);
        data.put_u16_le(16);
        // cbSize, not accounted for by the fmt chunk size above
        data.put_u16_le(0);
        data.put(&b"data"[..]);
        data.put_u32_le(120);
        let header_read = read_wav_header(&mut data).unwrap();
        assert_eq!(header_read.format_tag, 3);
        assert_eq!(header_read.extension_size, Some(0));
        assert_eq!(header_read.data_size, 120);
    }

    /// Broadcast Wave files put a "bext" chunk in front of the fmt chunk, other tools write
    /// "JUNK" padding there. Both have to survive a round trip in the original order.
    #[test]
    fn test_read_write_wav_header_chunks_before_fmt() {
        let mut data = BytesMut::new();
        data.put(&b"RIFF"[..]);
        data.put_u32_le(180);
        data.put(&b"WAVE"[..]);
        data.put(&b"JUNK"[..]);
        data.put_u32_le(4);
        data.put(&b"\0\0\0\0"[..]);
        data.put(&b"bext"[..]);
        data.put_u32_le(3);
        data.put(&b"abc"[..]);
        data.put_u8(0); // pad byte
        data.put(&b"fmt "[..]);
        data.put_u32_le(16);
        data.put_u16_le(1);
        data.put_u16_le(1);
        data.put_u32_le(22050);
        data.put_u32_le(44100);
        data.put_u16_le(2);
        data.put_u16_le(16);
        data.put(&b"data"[..]);
        data.put_u32_le(120);
        let expected = data.to_vec();

        let header_read = read_wav_header(&mut data).unwrap();
        assert_eq!(header_read.samples_per_sec, 22050);
        assert_eq!(header_read.data_size, 120);
        assert_eq!(
            header_read.pre_fmt_fields,
            b"JUNK\x04\x00\x00\x00\0\0\0\0bext\x03\x00\x00\x00abc\x00"
        );
        assert!(header_read.extra_fields.is_empty());

        let mut written = BytesMut::new();
        write_wav_header(&header_read, &mut written);
        assert_eq!(expected, written.to_vec());
    }

    /// A file without a fmt chunk can not be described by a WaveForm
    #[test]
    fn test_read_wav_header_missing_fmt_chunk() {
        let mut data = BytesMut::new();
        data.put(&b"RIFF"[..]);
        data.put_u32_le(28);
        data.put(&b"WAVE"[..]);
        data.put(&b"JUNK"[..]);
        data.put_u32_le(4);
        data.put(&b"\0\0\0\0"[..]);
        data.put(&b"data"[..]);
        data.put_u32_le(120);
        let error = read_wav_header(&mut data).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("no fmt chunk"), "{error}");
    }

    /// An odd sized chunk before the data chunk is followed by a pad byte
    #[test]
    fn test_read_wav_header_odd_sized_chunk() {
        let mut data = BytesMut::new();
        data.put(&b"RIFF"[..]);
        data.put_u32_le(168);
        data.put(&b"WAVE"[..]);
        data.put(&b"fmt "[..]);
        data.put_u32_le(16);
        data.put_u16_le(1);
        data.put_u16_le(1);
        data.put_u32_le(22050);
        data.put_u32_le(44100);
        data.put_u16_le(2);
        data.put_u16_le(16);
        data.put(&b"cue "[..]);
        data.put_u32_le(3);
        data.put(&b"abc"[..]);
        data.put_u8(0); // pad byte
        data.put(&b"data"[..]);
        data.put_u32_le(120);
        let header_read = read_wav_header(&mut data).unwrap();
        assert_eq!(header_read.data_size, 120);
        assert_eq!(header_read.extra_fields, b"cue \x03\x00\x00\x00abc\x00");
    }

    /// A bogus chunk size must not be trusted, u32::MAX would also overflow the pad byte
    /// calculation on 32 bit targets like wasm32
    #[test]
    fn test_read_wav_header_bogus_chunk_size() {
        let mut data = BytesMut::new();
        data.put(&b"RIFF"[..]);
        data.put_u32_le(48);
        data.put(&b"WAVE"[..]);
        data.put(&b"fmt "[..]);
        data.put_u32_le(16);
        data.put_u16_le(1);
        data.put_u16_le(1);
        data.put_u32_le(22050);
        data.put_u32_le(44100);
        data.put_u16_le(2);
        data.put_u16_le(16);
        data.put(&b"cue "[..]);
        data.put_u32_le(u32::MAX);
        data.put(&b"data"[..]);
        data.put_u32_le(120);
        let error = read_wav_header(&mut data).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_read_wav_header_truncated() {
        let mut data = BytesMut::from(&b"RIFFxxxxWAVEfmt "[..]);
        let error = read_wav_header(&mut data).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_read_wav_header_not_a_wav() {
        let mut data = BytesMut::from(&b"OggS0123456789abcdef"[..]);
        let error = read_wav_header(&mut data).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
