//! Port of VisualPinballEngine's LZW compression algorithm.
//! which is a port of VisualPinball's LZW compression algorithm.
//!
//! https://github.com/vpinball/vpinball/blob/master/media/lzwwriter.h
//! https://github.com/vpinball/vpinball/blob/master/media/lzwwriter.cpp
//!
//! https://github.com/freezy/VisualPinball.Engine/blob/master/VisualPinball.Engine/IO/LzwWriter.cs

// GIF Image compression - modified 'compress'
//
// Based on: compress.c - File compression ala IEEE Computer, June 1984.
//
// By Authors:  Spencer W. Thomas      (decvax!harpo!utah-cs!utah-gr!thomas)
//              Jim McKie              (decvax!mcvax!jim)
//              Steve Davies           (decvax!vax135!petsd!peora!srd)
//              Ken Turkowski          (decvax!decwrl!turtlevax!ken)
//              James A. Woods         (decvax!ihnp4!ames!jaw)
//              Joe Orost              (decvax!vax135!petsd!joe)

use crate::vpx::lzw_reader::LzwReader;
use byteorder::WriteBytesExt;
use std::io::Write;
use weezl::BitOrder;

const CODE_MASK: [u16; 13] = [
    0, 0x0001, 0x0003, 0x0007, 0x000F, 0x001F, 0x003F, 0x007F, 0x00FF, 0x01FF, 0x03FF, 0x07FF,
    0x0FFF,
];

const H_SIZE: usize = 5003; // 80% occupancy
const BITS: u16 = 12;
const MAX_BITS: u16 = BITS;
const MAX_MAX_CODE: u16 = 1 << BITS;

pub(crate) struct LzwWriter {
    compressed: Vec<u8>,
    bits: Vec<u8>,
    width: u32,
    height: u32,
    /// x-length of each scan line (divisible by 8, normally)
    pitch: u32,
    n_bits: u16,
    max_code: u16,
    h_tab: Vec<i32>,
    code_tab: Vec<u16>,
    free_ent: u16,
    init_bits: u16,
    clear_code: u16,
    eof_code: u16,
    cur_accum: u16,
    cur_bits: u16, // max of 12
    accum_count: usize,
    accum: Vec<u8>,
    i_pixel_cur: u32,
    i_x_cur: u32,
    clear_flg: bool,
}

impl LzwWriter {
    pub(crate) fn new(bits: Vec<u8>, width: u32, height: u32, bytes_per_pixel: u8) -> LzwWriter {
        let stride = width * bytes_per_pixel as u32;
        LzwWriter {
            compressed: vec![],
            bits,
            // strange that is the same as stride
            width: stride,
            height,
            pitch: stride,
            n_bits: 0,
            max_code: 0,

            // Hash map index to code
            h_tab: vec![0; H_SIZE],
            // Hash map code to index
            code_tab: vec![0; H_SIZE],
            // next available slot in hash table
            free_ent: 0,

            init_bits: 0,
            clear_code: 0,
            eof_code: 0,
            cur_accum: 0,
            cur_bits: 0,
            accum_count: 0,
            accum: vec![0; 256],
            i_pixel_cur: 0,
            i_x_cur: 0,
            clear_flg: false,
        }
    }

    fn max_code(n_bits: u16) -> u16 {
        (1 << n_bits) - 1
    }

    fn write_byte(&mut self, ch: u8) {
        self.compressed.write_u8(ch).unwrap();
    }

    fn next_pixel(&mut self) -> Option<u8> {
        if self.i_pixel_cur == self.pitch * self.height {
            return None;
        }

        let ch = self.bits[self.i_pixel_cur as usize];
        self.i_pixel_cur += 1;
        self.i_x_cur += 1;
        if self.i_x_cur == self.width {
            self.i_pixel_cur += self.pitch - self.width;
            self.i_x_cur = 0;
        }
        Some(ch)
    }

    pub(crate) fn compress_bits(&mut self, init_bits: u16) -> Vec<u8> {
        // Used to be in write gif
        self.i_pixel_cur = 0;
        self.i_x_cur = 0;

        self.clear_flg = false;

        self.cur_accum = 0;
        self.cur_bits = 0;

        self.accum_count = 0;

        // Set up the globals:  g_init_bits - initial number of bits
        // bits per pixel
        self.init_bits = init_bits;

        // Set up the necessary values
        self.n_bits = self.init_bits;
        self.max_code = Self::max_code(self.n_bits);

        self.clear_code = 1 << (init_bits - 1);
        self.eof_code = self.clear_code + 1;
        self.free_ent = self.clear_code + 2;

        println!(
            "clear_code: {}, eof_code: {}, free_ent: {}",
            self.clear_code, self.eof_code, self.free_ent
        );

        let mut ent = self.next_pixel().unwrap() as u16;

        let mut h_shift = 0;
        let mut f_code = H_SIZE;
        while f_code < 65536 {
            h_shift += 1;
            f_code *= 2;
        }
        h_shift = 8 - h_shift; // set hash code range bound

        self.hashtable_clear(H_SIZE);

        self.output(self.clear_code);

        enum LoopResult {
            ProcessByte,
            NextByte,
        }

        while let Some(next_pixel) = self.next_pixel() {
            let c = next_pixel as u32;
            let f_code = (c << MAX_BITS) + ent as u32;
            let mut i = ((c << h_shift) ^ ent as u32) as usize; // xor hashing

            // is first probed slot empty?
            let result = if self.hashtable_contains_hash(i) {
                if self.hashtable_contains(i, f_code) {
                    // hash code matches, no collision
                    ent = self.code_tab[i];
                    LoopResult::NextByte
                } else {
                    //println!("!!!collision at hash {}", i);
                    // linear probing
                    let disp = if i == 0 { 1 } else { H_SIZE - i };

                    let mut goto = None;
                    while goto.is_none() {
                        i = (i + H_SIZE - disp) % H_SIZE;

                        if self.code_tab[i] == 0 {
                            // hit empty slot
                            // goto processByte;
                            goto = Some(LoopResult::ProcessByte);
                        } else if self.hashtable_contains(i, f_code) {
                            ent = self.code_tab[i];
                            // goto nextByte;
                            goto = Some(LoopResult::NextByte);
                        }
                    }
                    goto.unwrap()
                }
            } else {
                LoopResult::ProcessByte
            };

            match result {
                LoopResult::ProcessByte => {
                    self.output(ent);
                    ent = c as u16;
                    if self.free_ent < MAX_MAX_CODE {
                        // code -> hashtable
                        self.hashtable_put(i, f_code);
                    } else {
                        self.clear_block();
                    }
                }
                LoopResult::NextByte => {
                    // do nothing
                }
            }
        }

        // self.hashtable_print();

        // Put out the final code.
        self.output(ent);
        self.output(self.eof_code);
        self.compressed.clone()
    }

    fn hashtable_print(&mut self) {
        for i in 0..H_SIZE {
            if self.code_tab[i] != 0 {
                let p_code = self.h_tab[i] as u32;
                let repr = Self::reconstruct_f_code_components(p_code);
                println!("hash {}: repr {:?} -> code {}", i, repr, self.code_tab[i]);
            }
        }

        // print the number of entries in the hash table
        let mut count = 0;
        for i in 0..H_SIZE {
            if self.code_tab[i] != 0 {
                count += 1;
            }
        }
        println!("Hash table count: {}", count);
    }

    fn reconstruct_f_code_components(mut f_code: u32) -> Vec<u32> {
        // reconstruct the data by shifting the code MAX_BITS to the right each time
        let mut repr = vec![];
        if f_code == 0 {
            repr.push(0);
        }
        // there are always 2 components in the table
        for _ in 0..2 {
            // take the last MAX_BITS bits
            repr.push(f_code & ((1 << MAX_BITS) - 1));
            f_code >>= MAX_BITS;
        }
        repr
    }

    fn output(&mut self, code: u16) {
        self.cur_accum &= CODE_MASK[self.cur_bits as usize];

        if self.cur_bits > 0 {
            self.cur_accum |= code << self.cur_bits;
        } else {
            self.cur_accum = code;
        }

        self.cur_bits += self.n_bits;

        while self.cur_bits >= 8 {
            self.char_out((self.cur_accum & 0xff) as u8);
            self.cur_accum >>= 8;
            self.cur_bits -= 8;
        }

        // If the next entry is going to be too big for the code size,
        // then increase it, if possible.
        if self.free_ent > self.max_code || self.clear_flg {
            if self.clear_flg {
                self.n_bits = self.init_bits;
                self.max_code = Self::max_code(self.n_bits);
                self.clear_flg = false;
            } else {
                self.n_bits += 1;
                self.max_code = if self.n_bits == MAX_BITS {
                    MAX_MAX_CODE
                } else {
                    Self::max_code(self.n_bits)
                };
            }
        }

        if code == self.eof_code {
            // At EOF, write the rest of the buffer.
            while self.cur_bits > 0 {
                self.char_out((self.cur_accum & 0xff) as u8);
                self.cur_accum >>= 8;
                if self.cur_bits > 8 {
                    self.cur_bits -= 8;
                } else {
                    self.cur_bits = 0;
                }
            }
            self.flush_char();
        }
    }

    fn clear_block(&mut self) {
        self.hashtable_clear(H_SIZE);
        self.free_ent = self.clear_code + 2;
        self.clear_flg = true;

        self.output(self.clear_code);
    }

    fn hashtable_contains_hash(&mut self, hash: usize) -> bool {
        self.h_tab[hash] != -1
    }

    fn hashtable_contains(&mut self, hash: usize, f_code: u32) -> bool {
        let f_code: i32 = f_code.try_into().unwrap();
        self.h_tab[hash] == f_code
    }

    fn hashtable_put(&mut self, hash: usize, f_code: u32) {
        self.code_tab[hash] = self.free_ent;
        self.h_tab[hash] = f_code.try_into().unwrap();
        self.free_ent += 1;
    }

    fn hashtable_clear(&mut self, h_size: usize) {
        for i in 0..h_size {
            self.h_tab[i] = -1;
            self.code_tab[i] = 0;
        }
    }

    fn char_out(&mut self, c: u8) {
        self.accum[self.accum_count] = c;
        self.accum_count += 1;
        // 254 is the maximum number of data bytes that can be
        // written in one block in the GIF file format.
        // A block starts with a byte that gives the number of
        // data bytes that follow, and then the data bytes
        if self.accum_count >= 254 {
            self.flush_char();
        }
    }

    /// TODO get this gif block writing out of here
    fn flush_char(&mut self) {
        if self.accum_count > 0 {
            self.write_byte(self.accum_count as u8);
            let sz = &self
                .accum
                .iter()
                .take(self.accum_count)
                .cloned()
                .collect::<Vec<u8>>();
            self.compressed.write_all(sz).unwrap();
            self.accum_count = 0;
        }
    }
}

/// Convert gif blocks to continuous bytes
/// We could optimize this in an iterator
fn from_blocks(uncompressed: &[u8]) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![];
    let mut iter = uncompressed.iter();
    while let Some(block_size) = iter.next() {
        let block_size = *block_size as usize;
        for _ in 0..block_size {
            bytes.push(*iter.next().unwrap());
        }
    }
    bytes
}

/// Convert bytes to gif blocks
/// We could optimize this in an iterator
/// This is the reverse of unblock
///
/// typically the max_block_len is 254
fn to_blocks(compressed: Vec<u8>, max_block_len: u8) -> Vec<u8> {
    let mut blocks: Vec<u8> = vec![];
    let mut iter = compressed.iter();
    let mut block: Vec<u8> = vec![];
    let mut block_len = 0;
    while let Some(byte) = iter.next() {
        block.push(*byte);
        block_len += 1;
        if block_len == max_block_len {
            blocks.push(block_len);
            blocks.append(&mut block);
            block_len = 0;
        }
    }
    if block_len > 0 {
        blocks.push(block_len);
        blocks.append(&mut block);
    }
    blocks
}

fn to_lzw(data: &[u8]) -> Vec<u8> {
    weezl::encode::Encoder::new(BitOrder::Lsb, 8)
        .encode(&data)
        .unwrap()
}

pub fn to_lzw_blocks(data: &[u8]) -> Vec<u8> {
    let compressed = to_lzw(&data);
    // convert compressed bytes to gif blocks
    to_blocks(compressed, 254)
}

pub fn to_lzw_blocks_old(data: &[u8], width: u32, height: u32, bytes_per_pixel: u8) -> Vec<u8> {
    let mut lzw_writer = LzwWriter::new(data.to_vec(), width, height, bytes_per_pixel);
    lzw_writer.compress_bits(8 + 1)
}

pub fn from_lzw_blocks(compressed: &[u8]) -> Vec<u8> {
    // convert gif blocks to compressed bytes
    let compressed = from_blocks(compressed);
    from_lzw(&compressed)
}

pub fn from_lzw_blocks_old(
    compressed: &[u8],
    width: u32,
    height: u32,
    bytes_per_pixel: u8,
) -> Vec<u8> {
    let mut lzw_reader = LzwReader::new(
        Box::new(std::io::Cursor::new(compressed.to_vec())),
        width,
        height,
        bytes_per_pixel,
    );
    lzw_reader.decompress()
}

fn from_lzw(compressed: &[u8]) -> Vec<u8> {
    let decompressed = weezl::decode::Decoder::new(BitOrder::Lsb, 8)
        .decode(&compressed)
        .unwrap();
    decompressed
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::collections::HashSet;

    fn lzw_blocks_to_codes(compressed: &[u8]) -> Vec<u16> {
        let unblocked = from_blocks(compressed);
        lzw_to_codes(&unblocked)
    }

    /// Convert compressed bytes to codes
    /// for debugging purposes
    ///
    /// Something is still messed up with this function
    fn lzw_to_codes(compressed: &[u8]) -> Vec<u16> {
        let mut codes: Vec<u16> = vec![];
        let start_code_width = 9;
        let mut code_width = start_code_width;

        let mut current_code: u16 = 0;
        let mut current_code_width: u8 = 0;
        let mut increase_width_on = 1 << (start_code_width - 1);

        let clear_code = 1 << (start_code_width - 1);
        let eof_code = clear_code + 1;
        let iter = compressed.iter();
        let mut unique_codes: HashSet<u16> = HashSet::new(); // to keep track of unique codes
        for byte in iter {
            // bits
            for bit in 0..8 {
                current_code |= ((byte >> bit) as u16 & 1) << current_code_width;
                current_code_width += 1;
                if current_code_width == code_width {
                    if current_code == clear_code {
                        println!(
                            "lzw_to_codes - clear code, reset code size to {}",
                            start_code_width
                        );
                        code_width = start_code_width;
                    } else {
                        unique_codes.insert(current_code);
                    }

                    codes.push(current_code);

                    //println!("code: {} unique_codes: {}", current_code, unique_codes.len());
                    if unique_codes.len() + 1 == increase_width_on {
                        if increase_width_on != 512 {
                            // The first 512 codes are reserved for single byte values
                            // and control codes (like the clear code and the end-of-information
                            // code). Therefore, when the number of unique codes reaches 512,
                            // the code width does not need to be increased because these codes
                            // can be represented within the current code width.
                            code_width += 1;
                            //println!("lzw_to_codes - {} unique codes - code width increased to {}", unique_codes.len(), code_width);
                        }
                        increase_width_on <<= 1;
                        assert!(code_width <= 12, "code_size should not exceed 12");
                    }
                    current_code = 0;
                    current_code_width = 0;
                }
            }
        }

        if current_code_width > 0 {
            // expect all 0s at the end
            assert_eq!(current_code, 0);
        }

        // expect eof code at the end, corrected to the correct code size
        assert_eq!(codes.last().unwrap(), &eof_code);

        codes
    }

    #[test]
    fn test_to_blocks_from_blocks() {
        let compressed = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let max_block_len = 3;
        let blocks = to_blocks(compressed.clone(), max_block_len);
        let uncompressed = from_blocks(&blocks);
        assert_eq!(uncompressed, compressed);
    }

    #[test]
    fn test_to_blocks_empty() {
        let compressed = vec![];
        let max_block_len = 3;
        let blocks = to_blocks(compressed.clone(), max_block_len);

        // should this be [0] ?
        assert_eq!(blocks, [0u8; 0]);
    }

    #[test]
    fn test_lzw_writer_four_0() {
        let bits = vec![0; 4];
        let width = 2;
        let height = 1;
        let bytes_per_pixel = 2;
        let compressed_blocks_old = to_lzw_blocks_old(&bits, width, height, bytes_per_pixel);
        assert_eq!(compressed_blocks_old, vec![6, 0, 1, 8, 4, 16, 16]);

        let codes_old = lzw_blocks_to_codes(&compressed_blocks_old);
        // 257 = eof code
        // 256 = clear code (reset dictionary)
        // 0 = 0
        // 258 = 0 0
        assert_eq!(codes_old, vec![256, 0, 258, 0, 257]);
        // which corresponds to 0 00 0 which was the input

        let compressed_blocks = to_lzw_blocks(&bits);
        assert_eq!(compressed_blocks, [6, 0, 1, 8, 4, 16, 16]);
    }

    #[test]
    fn test_lzw_writer_four_255() {
        let bits = vec![255; 4];
        let width = 2;
        let height = 1;
        let bytes_per_pixel = 2;
        let compressed = to_lzw_blocks_old(&bits, width, height, bytes_per_pixel);
        assert_eq!(compressed, [6, 0, 255, 9, 252, 23, 16]);

        let compressed_blocks = to_lzw_blocks(&bits);
        assert_eq!(compressed_blocks, [6, 0, 255, 9, 252, 23, 16]);

        let codes = lzw_blocks_to_codes(&compressed);
        // 257 = eof code
        // 256 = clear code (reset dictionary)

        // 255 = 255
        // 258 = 255 255
        assert_eq!(codes, [256, 255, 258, 255, 257]);
        // which corresponds to 255 255 255 255 which was the input
    }

    #[test]
    fn test_lzw_writer_minimal() {
        // we keep alpha channel at 0xAA because it will be dropped in other tests
        #[rustfmt::skip]
        let bits = vec![
            0xFF, 0xAA, 0xAA, 0xFF, // red
            0xAA, 0xFF, 0xAA, 0xFF, // green
            0xAA, 0xAA, 0xFF, 0xFF, // blue
            0xFF, 0xFF, 0xFF, 0xFF // white
        ];
        let width = 2;
        let height = 2;
        let bytes_per_pixel = 4;
        let compressed = to_lzw_blocks_old(&bits, width, height, bytes_per_pixel);
        assert_eq!(
            compressed,
            [13, 0, 255, 169, 82, 37, 176, 224, 192, 127, 8, 19, 6, 4,]
        );

        let compressed_blocks = to_lzw_blocks(&bits);
        assert_eq!(
            compressed_blocks,
            [13, 0, 255, 169, 82, 37, 176, 224, 192, 127, 8, 19, 6, 4,]
        );

        let codes = lzw_blocks_to_codes(&compressed);
        // 257 = eof code
        // 256 = clear code (reset dictionary)

        // 255 = 255 (0xFF)
        // 170 = 170 (0xAA)
        // 258 = 255 170
        // 261 = 255 170 170
        // 259 = 170 255
        // 264 = 255 170 170 255
        // 265 = 255 170 170 255 170

        assert_eq!(
            codes,
            [256, 255, 170, 170, 258, 261, 259, 255, 264, 265, 257]
        );
    }

    #[test]
    fn test_to_codes() {
        let end = 255;
        let bits: Vec<u8> = (0..=end).collect();

        let compressed_blocks = to_lzw_blocks_old(&bits, end as u32 + 1, 1, 1);
        let compressed = from_blocks(&compressed_blocks);

        let compressed2 = to_lzw(&bits);
        assert_eq!(compressed, compressed2);

        let codes = lzw_to_codes(&compressed);
        // 256 = clear code
        // no compression, so each byte is a code
        // 257 = eof code
        let expected: Vec<u16> = std::iter::once(256u16)
            .chain(0..=end as u16)
            .chain(std::iter::once(257u16))
            .collect();
        assert_eq!(codes, expected);
    }

    #[test]
    fn test_codes_zeroes() {
        // first one that makes the code size increase to 11
        let bits: Vec<u8> = vec![0; 200_029];
        let compressed = to_lzw(&bits);
        let codes = lzw_to_codes(&compressed);
        let unique_codes: HashSet<u16> = codes.iter().cloned().collect();
        assert_eq!(unique_codes.len(), 634);

        let compressed_old = to_lzw_blocks_old(&bits, 200_029, 1, 1);
        let codes_old = lzw_blocks_to_codes(&compressed_old);

        assert_eq!(codes, codes_old);
    }

    #[test]
    fn test_lzw_write_read() {
        let width: u32 = 222; //49 / 74;
        let height: u32 = 1;
        let bytes_per_pixel: u8 = 4;

        let bits_size = (width * height * bytes_per_pixel as u32) as usize;
        //println!("bits_size: {}", bits_size);
        let mut bits = vec![0; bits_size];
        for i in 0..width * height {
            bits[i as usize * bytes_per_pixel as usize] = (i % 256) as u8;
            bits[i as usize * bytes_per_pixel as usize + 1] = ((i + 1) % 256) as u8;
            bits[i as usize * bytes_per_pixel as usize + 2] = ((i + 2) % 256) as u8;
            //bits[i as usize * bytes_per_pixel as usize + 3] = ((i + 3) % 256) as u8;
        }

        let compressed_blocks = to_lzw_blocks(&bits);
        let decompressed = from_lzw_blocks(&compressed_blocks);
        assert_eq!(bits, decompressed);

        // let blocks = lzw_blocks_to_codes(&compressed_blocks);
        // println!("blocks: {:?}", blocks);

        let compressed = to_lzw_blocks_old(&bits, width, height, bytes_per_pixel);
        let blocks = lzw_blocks_to_codes(&compressed);
        println!("blocks: {:?}", blocks);

        assert_eq!(compressed, compressed_blocks);

        let decompressed = from_lzw_blocks_old(&compressed, width, height, bytes_per_pixel);
        assert_eq!(bits, decompressed);

        // let decompressed = from_lzw_blocks_old(&compressed, height, bytes_per_pixel, width);
        //
        // // compare last 10 bytes first
        // assert_eq!(&decompressed[bits_size - 10..], &bits[bits_size - 10..]);
        // assert_eq!(decompressed, bits);
    }

    #[test]
    fn test_lzw_read_write() {
        let width: u32 = 128;
        let height: u32 = 128;
        let bytes_per_pixel: u8 = 4;
        let file_path = "testdata/raw_lzw_bmp_128_128_data.bin";
        let compressed_original = std::fs::read(file_path).unwrap();

        //let codes_original = lzw_blocks_to_codes(&compressed_original);
        //println!("codes: {:?}", codes_original);

        let decompressed_old =
            from_lzw_blocks_old(&compressed_original, width, height, bytes_per_pixel);
        let decompressed = from_lzw_blocks(&compressed_original);

        assert_eq!(decompressed, decompressed_old);

        // let compressed_old = to_lzw_blocks_old(&decompressed, width, height, bytes_per_pixel);
        // let compressed = to_lzw_blocks(&decompressed);

        //let codes_old = lzw_blocks_to_codes(&compressed_old);
        //let codes = lzw_blocks_to_codes(&compressed);

        //assert_eq!(codes_old, codes_original);
        //assert_eq!(codes, codes_original);

        //assert_eq!(compressed_old, compressed_original);
        //assert_eq!(compressed, compressed_original);
    }
}
