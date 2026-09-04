use encoding_rs::mem::{decode_latin1, encode_latin1_lossy};
use log::warn;
use std::fmt;
use std::io;

use super::model::{StringEncoding, StringWithEncoding};
use super::utf16::{decode_utf16le, encode_utf16le};

pub trait BiffRead {
    fn biff_read(reader: &mut BiffReader<'_>) -> Self;
}

pub trait BiffWrite {
    fn biff_write(&self, writer: &mut BiffWriter);
}

/// A structural problem in a BIFF stream: truncated data, a record that
/// claims more bytes than the stream holds, a missing `ENDB`, ...
///
/// Reading never panics on such input. The [`BiffReader`] records the first
/// problem it hits, moves to the end of the stream so every following read
/// yields a default value, and the caller checks [`BiffReader::check`] once
/// the item is read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiffError {
    pub message: String,
    /// Tag of the record being read when the problem was found, empty if none
    pub tag: String,
    /// Byte offset in the stream where the problem was found
    pub pos: usize,
    /// Length of the stream
    pub len: usize,
}

impl fmt::Display for BiffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.tag.is_empty() {
            write!(
                f,
                "{} (at offset {} of {} bytes)",
                self.message, self.pos, self.len
            )
        } else {
            write!(
                f,
                "{} (tag {:?} at offset {} of {} bytes)",
                self.message, self.tag, self.pos, self.len
            )
        }
    }
}

impl std::error::Error for BiffError {}

impl From<BiffError> for io::Error {
    fn from(e: BiffError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, e)
    }
}

pub struct BiffReader<'a> {
    data: &'a [u8],
    pos: usize,
    bytes_in_record_remaining: usize,
    record_start: usize,
    tag: String,
    warn_remaining: bool,
    error: Option<BiffError>,
}
// TODO make private
/**
 * All records have a tag, eg CODE or NAME
 */
pub const RECORD_TAG_LEN: u32 = 4;

pub const WARN: bool = true;

/// Tag the reader reports once the end of the stream is reached
const END_TAG: &str = "ENDB";

impl<'a> BiffReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        BiffReader::with_remaining(data, 0)
    }

    pub fn with_remaining(data: &'a [u8], bytes_in_record_remaining: usize) -> Self {
        BiffReader {
            data,
            pos: 0,
            bytes_in_record_remaining,
            record_start: 0,
            tag: "".to_string(),
            warn_remaining: true,
            error: None,
        }
    }

    /**
     * Useful if you just want to read a bunch of tags and don't care about the data
     */
    pub fn disable_warn_remaining(&mut self) {
        self.warn_remaining = false;
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn tag(&self) -> String {
        self.tag.to_string()
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.data.len() || self.tag == END_TAG
    }

    /// The first structural problem found in the stream, if any
    pub fn error(&self) -> Option<&BiffError> {
        self.error.as_ref()
    }

    /// Fails if a structural problem was found while reading the stream
    pub fn check(&self) -> Result<(), BiffError> {
        match &self.error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }

    /// Record a structural problem and move to the end of the stream.
    ///
    /// Only the first problem is kept. After this call [`Self::is_eof`] is
    /// true, [`Self::next`] returns `None` and every getter returns a
    /// default value, so read loops terminate without panicking.
    pub fn fail(&mut self, message: impl Into<String>) {
        if self.error.is_none() {
            self.error = Some(BiffError {
                message: message.into(),
                tag: self.tag.clone(),
                pos: self.pos,
                len: self.data.len(),
            });
        }
        self.pos = self.data.len();
        self.bytes_in_record_remaining = 0;
        self.tag = END_TAG.to_string();
    }

    /// Take over the problem found by a child reader, if any
    pub fn absorb(&mut self, child: &BiffReader<'_>) {
        if let Some(e) = &child.error {
            let message = e.message.clone();
            self.fail(message);
        }
    }

    /// Take `count` bytes from the stream, failing if not enough are left
    fn take(&mut self, count: usize) -> &'a [u8] {
        let available = self.data.len().saturating_sub(self.pos);
        if count > available {
            self.fail(format!(
                "{count} bytes requested, {available} left in stream"
            ));
            return &[];
        }
        let p = self.pos;
        self.pos += count;
        &self.data[p..p + count]
    }

    /// Take `count` bytes from the current record, failing if the record or
    /// the stream does not hold that many
    fn take_in_record(&mut self, count: usize) -> &'a [u8] {
        if count > self.bytes_in_record_remaining {
            self.fail(format!(
                "{count} bytes requested, {} left in record",
                self.bytes_in_record_remaining
            ));
            return &[];
        }
        let d = self.take(count);
        if d.len() == count {
            self.bytes_in_record_remaining -= count;
        }
        d
    }

    fn array<const N: usize>(d: &[u8]) -> [u8; N] {
        let mut a = [0u8; N];
        if d.len() == N {
            a.copy_from_slice(d);
        }
        a
    }

    pub fn get(&mut self, count: usize) -> &[u8] {
        self.take_in_record(count)
    }

    pub fn get_no_remaining_update(&mut self, count: usize) -> &[u8] {
        self.take(count)
    }

    pub fn remaining_in_record(&mut self) -> usize {
        self.bytes_in_record_remaining
    }

    pub fn get_bool(&mut self) -> bool {
        let all = self.take_in_record(4);
        if all.is_empty() {
            return false;
        }
        // Match VPX permissive behavior: log and treat nonzero as true.
        if all != [0, 0, 0, 0] && all != [1, 0, 0, 0] {
            warn!(
                "Unexpected bytes for tag {} bool: {all:?}. Treating as nonzero=true. Probably caused by an uninitialized bool field in vpinball.",
                self.tag
            );
        }
        all != [0, 0, 0, 0]
    }

    pub fn get_u8(&mut self) -> u8 {
        Self::array::<1>(self.take_in_record(1))[0]
    }

    pub fn get_u8_no_remaining_update(&mut self) -> u8 {
        Self::array::<1>(self.take(1))[0]
    }

    pub fn get_u16(&mut self) -> u16 {
        u16::from_le_bytes(Self::array(self.take_in_record(2)))
    }

    pub fn get_u16_no_remaining_update(&mut self) -> u16 {
        u16::from_le_bytes(Self::array(self.take(2)))
    }

    pub fn get_u32(&mut self) -> u32 {
        u32::from_le_bytes(Self::array(self.take_in_record(4)))
    }

    pub fn get_u32_no_remaining_update(&mut self) -> u32 {
        u32::from_le_bytes(Self::array(self.take(4)))
    }

    pub fn get_32(&mut self) -> i32 {
        self.get_i32()
    }

    pub fn get_32_no_remaining_update(&mut self) -> i32 {
        i32::from_le_bytes(Self::array(self.take(4)))
    }

    pub fn get_f32(&mut self) -> f32 {
        let data = self.take_in_record(4);
        let res = f32::from_le_bytes(Self::array(data));
        if res.is_nan() {
            warn!("NaN value found for tag {} f32: {data:?}", self.tag);
        }
        res
    }

    /// Decode a 0-terminated latin1 string from a fixed size buffer
    fn decode_cstr(data: &[u8]) -> String {
        let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
        decode_latin1(&data[..end]).to_string()
    }

    pub fn get_str(&mut self, count: usize) -> String {
        Self::decode_cstr(self.take_in_record(count))
    }

    pub fn get_str_with_encoding_no_remaining_update(
        &mut self,
        count: usize,
    ) -> StringWithEncoding {
        // Below is the code used to read the CODE field in the C++ version
        //
        //    // check if script is either plain ASCII or UTF-8, or if it contains invalid stuff
        //    uint32_t state = UTF8_ACCEPT;
        //    if (validate_utf8(&state, szText, cchar) == UTF8_REJECT) {
        //       char* const utf8Text = iso8859_1_to_utf8(szText, cchar); // old ANSI characters? -> convert to UTF-8
        //       delete[] szText;
        //       szText = utf8Text;
        //    }
        //
        // https://github.com/vpinball/vpinball/blob/5ac9cfcb19e721ed9373465866cb724a655ad55f/codeview.cpp#L1761-L1767

        let data = self.take(count);
        let end = data.iter().position(|b| *b == 0).unwrap_or(data.len());
        let s: StringWithEncoding = data[..end].into();
        s
    }

    pub fn get_str_no_remaining_update(&mut self, count: usize) -> String {
        Self::decode_cstr(self.take(count))
    }

    pub fn get_string(&mut self) -> String {
        let size = self.get_u32() as usize;
        self.get_str(size)
    }

    pub fn get_string_no_remaining_update(&mut self) -> String {
        let size = self.get_u32_no_remaining_update() as usize;
        self.get_str_no_remaining_update(size)
    }

    pub fn get_wide_string(&mut self) -> String {
        let count = self.get_u32() as usize;
        let data = self.take_in_record(count);
        match decode_utf16le(data) {
            Ok(s) => s,
            Err(e) => {
                self.fail(format!("Invalid utf16le string: {e}"));
                String::new()
            }
        }
    }

    #[deprecated]
    pub fn get_color(&mut self, has_alpha: bool) -> (f32, f32, f32, f32) {
        if has_alpha {
            (
                self.get_u8() as f32 / 255.0,
                self.get_u8() as f32 / 255.0,
                self.get_u8() as f32 / 255.0,
                self.get_u8() as f32 / 255.0,
            )
        } else {
            (
                self.get_u8() as f32 / 255.0,
                self.get_u8() as f32 / 255.0,
                self.get_u8() as f32 / 255.0,
                1.0,
            )
        }
    }

    pub fn get_double(&mut self) -> f64 {
        f64::from_le_bytes(Self::array(self.take_in_record(8)))
    }

    pub fn get_i16(&mut self) -> i16 {
        i16::from_le_bytes(Self::array(self.take_in_record(2)))
    }

    pub fn get_i32(&mut self) -> i32 {
        i32::from_le_bytes(Self::array(self.take_in_record(4)))
    }

    pub fn get_i64(&mut self) -> i64 {
        i64::from_le_bytes(Self::array(self.take_in_record(8)))
    }

    pub fn get_u64(&mut self) -> u64 {
        u64::from_le_bytes(Self::array(self.take_in_record(8)))
    }

    pub fn get_u32_array(&mut self, count: usize) -> Vec<u32> {
        (0..count).map(|_| self.get_u32()).collect()
    }

    pub fn get_u16_array(&mut self, count: usize) -> Vec<u16> {
        (0..count).map(|_| self.get_u16()).collect()
    }

    pub fn get_i16_array(&mut self, count: usize) -> Vec<i16> {
        (0..count).map(|_| self.get_i16()).collect()
    }

    pub fn get_i32_array(&mut self, count: usize) -> Vec<i32> {
        (0..count).map(|_| self.get_i32()).collect()
    }

    pub fn get_i64_array(&mut self, count: usize) -> Vec<i64> {
        (0..count).map(|_| self.get_i64()).collect()
    }

    pub fn get_u64_array(&mut self, count: usize) -> Vec<u64> {
        (0..count).map(|_| self.get_u64()).collect()
    }

    pub fn get_f32_array(&mut self, count: usize) -> Vec<f32> {
        (0..count).map(|_| self.get_f32()).collect()
    }

    pub fn get_f64_array(&mut self, count: usize) -> Vec<f64> {
        (0..count).map(|_| self.get_double()).collect()
    }

    pub fn get_string_array(&mut self, count: usize) -> Vec<String> {
        (0..count).map(|_| self.get_string()).collect()
    }

    pub fn get_record_data(&mut self, with_tag: bool) -> Vec<u8> {
        let remaining = self.bytes_in_record_remaining;
        if with_tag {
            let Some(start) = self.pos.checked_sub(RECORD_TAG_LEN as usize) else {
                self.fail("No record tag to include");
                return Vec::new();
            };
            let d = self.take(remaining);
            if d.is_empty() && remaining > 0 {
                return Vec::new();
            }
            self.bytes_in_record_remaining = 0;
            self.data[start..self.pos].to_vec()
        } else {
            let d = self.take(remaining).to_vec();
            self.bytes_in_record_remaining = 0;
            d
        }
    }

    pub fn get_data_no_remaining_update(&mut self) -> Vec<u8> {
        let len = self.get_u32_no_remaining_update() as usize;
        let data = self.take(len).to_vec();
        self.bytes_in_record_remaining = 0;
        data
    }

    pub fn get_data(&mut self, count: usize) -> &[u8] {
        let d = self.take(count);
        self.bytes_in_record_remaining = 0;
        d
    }

    pub(crate) fn get_remaining(&self) -> &[u8] {
        &self.data[self.pos..]
    }

    pub fn skip(&mut self, count: usize) {
        self.take_in_record(count);
    }

    pub fn skip_end_tag(&mut self, count: usize) {
        self.take(count);
        self.bytes_in_record_remaining = 0;
    }

    pub fn skip_tag(&mut self) -> usize {
        let remaining = self.bytes_in_record_remaining;
        self.take(remaining);
        self.bytes_in_record_remaining = 0;
        remaining
    }

    pub fn next(&mut self, warn: bool) -> Option<String> {
        if self.error.is_some() {
            return None;
        }
        if self.bytes_in_record_remaining > 0 {
            if warn {
                warn!(
                    "{} : {} unread octets",
                    self.tag, self.bytes_in_record_remaining
                );
            }
            self.skip(self.bytes_in_record_remaining);
            if self.error.is_some() {
                return None;
            }
        }
        self.record_start = self.pos;
        if self.pos >= self.data.len() {
            self.fail("Unexpected end of biff stream while reading next tag. Missing ENDB?");
            return None;
        }
        let record_size = self.get_u32_no_remaining_update() as usize;
        let tag = Self::decode_cstr(self.take(RECORD_TAG_LEN as usize));
        if self.error.is_some() {
            return None;
        }
        if record_size < RECORD_TAG_LEN as usize {
            self.fail(format!(
                "Record size {record_size} of tag {tag:?} is smaller than the tag itself"
            ));
            return None;
        }
        if tag.is_empty() {
            self.fail("Empty tag");
            return None;
        }
        let remaining = record_size - RECORD_TAG_LEN as usize;
        let available = self.data.len() - self.pos;
        if remaining > available {
            self.fail(format!(
                "Record {tag:?} claims {remaining} bytes, {available} left in stream"
            ));
            return None;
        }
        self.bytes_in_record_remaining = remaining;
        self.tag = tag;
        if self.tag == END_TAG {
            if self.warn_remaining && self.pos < self.data.len() {
                self.fail(format!(
                    "{} remaining bytes after ENDB",
                    self.data.len() - self.pos
                ));
            }
            return None;
        }
        if self.pos >= self.data.len() {
            // a record other than ENDB ends the stream
            self.fail("Unexpected end of biff stream after last record. Missing ENDB?");
            return None;
        }
        Some(self.tag.clone())
    }

    /// A reader over the rest of the stream, for records that embed another
    /// BIFF stream. Call [`Self::absorb`] afterwards so a problem found by
    /// the child is not lost.
    pub fn child_reader(&mut self) -> BiffReader<'a> {
        let data: &'a [u8] = self.data;
        BiffReader {
            data: &data[self.pos.min(data.len())..],
            pos: 0,
            bytes_in_record_remaining: 0,
            record_start: 0,
            tag: "".to_string(),
            warn_remaining: false,
            error: None,
        }
    }

    pub fn data_until(&mut self, tag: &[u8]) -> Vec<u8> {
        // read bytes until we see tag and return it, put pos to the beginning of the tag
        let found = (self.pos..self.data.len().saturating_sub(tag.len()) + 1)
            .find(|p| &self.data[*p..*p + tag.len()] == tag);
        let Some(pos) = found else {
            self.fail(format!("Tag {:?} not found", String::from_utf8_lossy(tag)));
            return Vec::new();
        };
        // go back one u32 to the tag size
        let Some(pos) = pos.checked_sub(RECORD_TAG_LEN as usize) else {
            self.fail(format!(
                "Tag {:?} found without a preceding size",
                String::from_utf8_lossy(tag)
            ));
            return Vec::new();
        };
        if pos < self.pos {
            self.fail(format!(
                "Tag {:?} found before the current position",
                String::from_utf8_lossy(tag)
            ));
            return Vec::new();
        }
        let data = self.data[self.pos..pos].to_vec();
        self.pos = pos;
        self.bytes_in_record_remaining = 0;
        data
    }
}

pub struct BiffWriter {
    data: Vec<u8>,
    tag_start: usize,
    tag: String,
    record_size: usize,
}

impl Default for BiffWriter {
    fn default() -> Self {
        BiffWriter {
            data: Vec::new(),
            tag_start: 0,
            tag: "".to_string(),
            record_size: 0,
        }
    }
}

impl BiffWriter {
    pub fn new() -> BiffWriter {
        BiffWriter::default()
    }

    pub fn get_data(&self) -> &[u8] {
        &self.data
    }

    pub fn end_tag(&mut self) {
        if !self.tag.is_empty() {
            //let length = self.data.len();
            let length: &u32 = &self.record_size.try_into().unwrap();
            let length_bytes = length.to_le_bytes();
            self.data[self.tag_start..self.tag_start + 4].copy_from_slice(&length_bytes);
            self.tag = "".to_string();
        }
    }

    pub fn end_tag_no_size(&mut self) {
        if !self.tag.is_empty() {
            let length: u32 = 4;
            let length_bytes = length.to_le_bytes();
            self.data[self.tag_start..self.tag_start + 4].copy_from_slice(&length_bytes);
            self.tag = "".to_string();
        }
    }

    pub fn new_tag(&mut self, tag: &str) {
        self.end_tag();
        self.tag_start = self.data.len();
        self.data.extend_from_slice(&[0, 0, 0, 0]); // placeholder for record size
        let tag_bytes = tag.as_bytes();
        // some tags are smaller than 4 characters, so we need to pad them
        let mut padded_tag_bytes = [0; 4];
        padded_tag_bytes[..tag_bytes.len()].copy_from_slice(tag_bytes);
        self.data.extend_from_slice(&padded_tag_bytes);
        self.tag = tag.to_string();
        self.record_size = 4;
    }

    pub fn write_u8(&mut self, value: u8) {
        self.record_size += 1;
        self.data.push(value);
    }

    pub fn write_8(&mut self, value: i8) {
        self.record_size += 1;
        self.data.push(value as u8);
    }

    pub fn write_u16(&mut self, value: u16) {
        self.record_size += 2;
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_16(&mut self, value: i16) {
        self.record_size += 2;
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u32(&mut self, value: u32) {
        self.record_size += 4;
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_32(&mut self, value: i32) {
        self.record_size += 4;
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_f32(&mut self, value: f32) {
        self.record_size += 4;
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_short_string(&mut self, value: &str) {
        let d = encode_latin1_lossy(value);
        self.write_u8(d.len().try_into().unwrap());
        self.write_data(&d);
    }

    pub fn write_string(&mut self, value: &str) {
        let d = encode_latin1_lossy(value);
        self.write_u32(d.len().try_into().unwrap());
        self.write_data(&d);
    }

    pub fn write_string_with_encoding(&mut self, value: &StringWithEncoding) {
        let d = match value.encoding {
            StringEncoding::Latin1 => encode_latin1_lossy(&value.string).to_vec(),
            StringEncoding::Utf8 => value.string.clone().into_bytes(),
        };
        self.write_u32(d.len().try_into().unwrap());
        self.write_data(&d);
    }

    pub fn write_string_empty_zero(&mut self, value: &str) {
        if value.is_empty() {
            // sound files encode empty string like this
            self.write_u32(1);
            self.write_u8(0);
        } else {
            self.write_string(value);
        }
    }

    pub fn write_wide_string(&mut self, value: &str) {
        let d = encode_utf16le(value);
        self.write_u32(d.len().try_into().unwrap());
        self.write_data(&d);
    }

    pub fn write_bool(&mut self, value: bool) {
        if value {
            self.write_u32(0x00000001);
        } else {
            self.write_u32(0x00000000);
        }
    }

    pub fn write_length_prefixed_data(&mut self, value: &[u8]) {
        self.write_u32(value.len().try_into().unwrap());
        self.write_data(value);
    }

    pub fn write_data(&mut self, value: &[u8]) {
        self.record_size += value.len();
        self.data.extend_from_slice(value);
    }

    pub fn write_tagged_empty(&mut self, tag: &str) {
        self.new_tag(tag);
        self.end_tag();
    }

    pub fn write_tagged_bool(&mut self, tag: &str, value: bool) {
        self.new_tag(tag);
        self.write_bool(value);
        self.end_tag();
    }

    pub fn write_tagged_f32(&mut self, tag: &str, value: f32) {
        self.new_tag(tag);
        self.write_f32(value);
        self.end_tag();
    }

    pub fn write_tagged_u32(&mut self, tag: &str, value: u32) {
        self.new_tag(tag);
        self.write_u32(value);
        self.end_tag();
    }

    pub fn write_tagged_i32(&mut self, tag: &str, value: i32) {
        self.new_tag(tag);
        self.write_32(value);
        self.end_tag();
    }

    pub fn write_tagged_string(&mut self, tag: &str, value: &str) {
        self.new_tag(tag);
        self.write_string(value);
        self.end_tag();
    }

    pub fn write_tagged_string_no_size(&mut self, tag: &str, value: &str) {
        self.new_tag(tag);
        self.write_string(value);
        self.end_tag_no_size();
    }

    pub fn write_tagged_string_with_encoding_no_size(
        &mut self,
        tag: &str,
        value: &StringWithEncoding,
    ) {
        self.new_tag(tag);
        self.write_string_with_encoding(value);
        self.end_tag_no_size();
    }

    pub fn write_tagged_wide_string(&mut self, tag: &str, value: &str) {
        self.new_tag(tag);
        self.write_wide_string(value);
        self.end_tag();
    }

    pub fn write_tagged_vec2(&mut self, tag: &str, x: f32, y: f32) {
        self.new_tag(tag);
        self.write_f32(x);
        self.write_f32(y);
        self.end_tag();
    }

    pub fn write_tagged_padded_vector(&mut self, tag: &str, x: f32, y: f32, z: f32) {
        self.new_tag(tag);
        self.write_f32(x);
        self.write_f32(y);
        self.write_f32(z);
        self.write_f32(0.0);
        self.end_tag();
    }

    /// Writes a 3-float vector without trailing padding. Used by tags
    /// like `BMIN` / `BMAX` whose chunks are exactly 12 bytes (vpinball
    /// reads them as `Vector3` / `AsVector3()`).
    pub fn write_tagged_unpadded_vector(&mut self, tag: &str, x: f32, y: f32, z: f32) {
        self.new_tag(tag);
        self.write_f32(x);
        self.write_f32(y);
        self.write_f32(z);
        self.end_tag();
    }

    pub fn write_tagged_data(&mut self, tag: &str, value: &[u8]) {
        self.new_tag(tag);
        self.write_data(value);
        self.end_tag();
    }

    pub fn write_tagged_data_without_size(&mut self, tag: &str, value: &[u8]) {
        self.new_tag(tag);
        self.write_data(value);
        self.end_tag_no_size();
    }

    pub fn write_tagged<T: BiffWrite>(&mut self, tag: &str, value: &T) {
        self.new_tag(tag);
        BiffWrite::biff_write(value, self);
        self.end_tag();
    }

    pub fn write_tagged_without_size<T: BiffWrite>(&mut self, tag: &str, value: &T) {
        self.new_tag(tag);
        BiffWrite::biff_write(value, self);
        self.end_tag_no_size();
    }

    pub fn write_tagged_with<T>(&mut self, tag: &str, value: &T, f: fn(&T, &mut BiffWriter) -> ()) {
        self.new_tag(tag);
        f(value, self);
        self.end_tag();
    }

    pub fn close(&mut self, write_endb: bool) {
        if write_endb {
            self.new_tag("ENDB");
        }
        self.end_tag();
    }

    pub(crate) fn write_marker_tag(&mut self, tag: &str) {
        self.new_tag(tag);
        self.end_tag();
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn read_write_empty() {
        let mut writer = BiffWriter::new();
        writer.close(true);
        let mut reader = BiffReader::new(writer.get_data());
        assert_eq!(reader.next(false), None);
        assert_eq!(reader.is_eof(), true);
    }

    #[test]
    fn read_write_empty_tag() {
        let mut writer = BiffWriter::new();
        writer.write_tagged_empty("TEST");
        writer.close(true);
        let mut reader = BiffReader::new(writer.get_data());
        assert_eq!(reader.next(false), Some("TEST".to_string()));
        reader.next(false);
        assert_eq!(reader.is_eof(), true);
    }
}

#[cfg(test)]
mod corrupt_input_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn stream_with(tag: &str, data: &[u8]) -> Vec<u8> {
        let mut writer = BiffWriter::new();
        writer.write_tagged_data(tag, data);
        writer.close(true);
        writer.get_data().to_vec()
    }

    #[test]
    fn truncated_stream_fails_without_panicking() {
        let bytes = stream_with("ABCD", &[1, 2, 3, 4, 5, 6, 7, 8]);
        for len in 0..bytes.len() {
            let mut reader = BiffReader::new(&bytes[..len]);
            while reader.next(false).is_some() {
                reader.get_u32();
                reader.get_u32();
            }
            assert!(reader.check().is_err(), "prefix of {len} bytes should fail");
            assert!(reader.is_eof());
        }
        let mut reader = BiffReader::new(&bytes);
        assert_eq!(reader.next(false), Some("ABCD".to_string()));
        assert_eq!(reader.get_u32(), 0x0403_0201);
        assert_eq!(reader.next(false), None);
        assert!(reader.check().is_ok());
    }

    #[test]
    fn record_larger_than_stream() {
        let mut bytes = 100u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"ABCD");
        let mut reader = BiffReader::new(&bytes);
        assert_eq!(reader.next(false), None);
        assert_eq!(reader.get_u32_array(50), vec![0; 50]);
        let err = reader.check().unwrap_err();
        assert!(err.message.contains("left in stream"), "{err}");
    }

    #[test]
    fn read_past_record_end() {
        let bytes = stream_with("ABCD", &[1, 2, 3, 4]);
        let mut reader = BiffReader::new(&bytes);
        reader.next(false);
        assert_eq!(reader.get_u32(), 0x0403_0201);
        assert_eq!(reader.get_u32(), 0);
        let err = reader.check().unwrap_err();
        assert!(err.message.contains("left in record"), "{err}");
        assert_eq!(reader.next(false), None);
    }

    #[test]
    fn missing_endb() {
        let mut writer = BiffWriter::new();
        writer.write_tagged_u32("ABCD", 1);
        let bytes = writer.get_data().to_vec();
        let mut reader = BiffReader::new(&bytes);
        assert_eq!(reader.next(false), Some("ABCD".to_string()));
        reader.get_u32();
        assert_eq!(reader.next(false), None);
        let err = reader.check().unwrap_err();
        assert!(err.message.contains("Missing ENDB"), "{err}");
    }

    #[test]
    fn data_after_endb() {
        let mut bytes = stream_with("ABCD", &[0; 4]);
        bytes.extend_from_slice(&[0xAA; 3]);
        let mut reader = BiffReader::new(&bytes);
        reader.next(false);
        reader.get_u32();
        assert_eq!(reader.next(false), None);
        let err = reader.check().unwrap_err();
        assert!(err.message.contains("after ENDB"), "{err}");
    }

    #[test]
    fn record_size_smaller_than_tag() {
        let mut bytes = 2u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"ABCD");
        let mut reader = BiffReader::new(&bytes);
        assert_eq!(reader.next(false), None);
        assert!(reader.check().is_err());
    }

    #[test]
    fn empty_tag() {
        let mut bytes = 4u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        let mut reader = BiffReader::new(&bytes);
        assert_eq!(reader.next(false), None);
        let err = reader.check().unwrap_err();
        assert!(err.message.contains("Empty tag"), "{err}");
    }

    #[test]
    fn oversized_string_length() {
        let mut writer = BiffWriter::new();
        writer.new_tag("NAME");
        writer.write_u32(u32::MAX);
        writer.write_data(b"ab");
        writer.end_tag();
        writer.close(true);

        let mut reader = BiffReader::new(writer.get_data());
        reader.next(false);
        assert_eq!(reader.get_string(), "");
        assert!(reader.check().is_err());

        let mut reader = BiffReader::new(writer.get_data());
        reader.next(false);
        assert_eq!(reader.get_wide_string(), "");
        assert!(reader.check().is_err());
    }

    #[test]
    fn data_until_missing_tag() {
        let bytes = stream_with("BITS", &[1, 2, 3]);
        let mut reader = BiffReader::new(&bytes);
        reader.next(false);
        assert_eq!(reader.data_until(b"ALTV"), Vec::<u8>::new());
        assert!(reader.check().is_err());
    }

    #[test]
    fn child_reader_error_is_absorbed() {
        let mut bytes = 50u32.to_le_bytes().to_vec();
        bytes.extend_from_slice(b"INNR");
        let mut parent = BiffReader::new(&bytes);
        let mut child = parent.child_reader();
        assert_eq!(child.next(false), None);
        assert!(child.check().is_err());
        assert!(parent.check().is_ok());
        parent.absorb(&child);
        assert!(parent.check().is_err());
        assert!(parent.is_eof());
    }

    #[test]
    fn error_converts_to_invalid_data_io_error() {
        let bytes = stream_with("ABCD", &[1, 2, 3, 4]);
        let mut reader = BiffReader::new(&bytes);
        reader.next(false);
        reader.get_double();
        let err = reader.check().unwrap_err();
        let io_err: io::Error = err.into();
        assert_eq!(io_err.kind(), io::ErrorKind::InvalidData);
        assert!(io_err.to_string().contains("ABCD"), "{io_err}");
    }
}
