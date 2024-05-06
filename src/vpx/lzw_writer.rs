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

use byteorder::WriteBytesExt;
use std::io::Write;

const CODE_MASK: [u16; 13] = [
    0, 0x0001, 0x0003, 0x0007, 0x000F, 0x001F, 0x003F, 0x007F, 0x00FF, 0x01FF, 0x03FF, 0x07FF,
    0x0FFF,
];

const H_SIZE: usize = 5003; // 80% occupancy
const BITS: u16 = 12;
const MAX_BITS: u16 = BITS;
const MAX_MAX_CODE: u16 = 1 << BITS;
const GIF_EOF: i32 = -1;

struct LzwWriter {
    compressed: Vec<u8>,
    bits: Vec<u8>,
    width: u32,
    height: u32,
    /// x-length of each scan line (divisible by 8, normally)
    pitch: u32,
    n_bits: u16,
    max_code: u16,
    h_tab: Vec<u16>,
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
    fn new(bits: Vec<u8>, width: u32, height: u32, bytes_per_pixel: u8) -> LzwWriter {
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
            h_tab: vec![0; H_SIZE],
            code_tab: vec![0; H_SIZE],
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

    fn write_sz(&mut self, sz: &[u8], num_bytes: usize) {
        self.compressed
            .write(&sz.iter().take(num_bytes).cloned().collect::<Vec<u8>>())
            .unwrap();
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

    fn compress_bits(&mut self, init_bits: u16) -> Vec<u8> {
        let mut c: i32;

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

        let mut ent = self.next_pixel().unwrap() as u16;

        let mut h_shift = 0;
        let mut f_code = H_SIZE;
        while f_code < 65536 {
            h_shift += 1;
            f_code *= 2;
        }

        h_shift = 8 - h_shift; // set hash code range bound

        self.clear_hash(H_SIZE);

        self.output(self.clear_code);

        while let Some(np) = self.next_pixel() {
            c = np as i32;
            let f_code = ((c as u16) << MAX_BITS) + ent;
            let mut i = ((c as u16) << h_shift) ^ ent; // xor hashing

            enum LoopResult {
                ProcessByte,
                NextByte,
            }

            // is first probed slot empty?
            let result = if self.code_tab[i as usize] != 0 {
                if self.h_tab[i as usize] == f_code {
                    ent = self.code_tab[i as usize];
                    LoopResult::NextByte
                } else {
                    let mut disp;
                    if i == 0 {
                        disp = 1;
                    } else {
                        disp = H_SIZE as u16 - i;
                    }

                    let mut loop_result = None;
                    while loop_result.is_none() {
                        i -= disp;
                        if i < 0 {
                            i += H_SIZE as u16;
                        }

                        if self.code_tab[i as usize] == 0 {
                            // goto processByte;
                            loop_result = Some(LoopResult::ProcessByte);
                        } else if self.h_tab[i as usize] == f_code {
                            ent = self.code_tab[i as usize];
                            // goto nextByte;
                            loop_result = Some(LoopResult::NextByte);
                        }
                    }
                    loop_result.unwrap()
                }
            } else {
                LoopResult::ProcessByte
            };

            match result {
                LoopResult::ProcessByte => {
                    self.output(ent);
                    ent = c as u16;
                    if self.free_ent < MAX_MAX_CODE {
                        self.code_tab[i as usize] = self.free_ent;
                        self.h_tab[i as usize] = f_code;
                        self.free_ent += 1;
                    } else {
                        self.clear_block();
                    }
                }
                LoopResult::NextByte => {
                    // do nothing
                }
            }
        }

        self.output(ent);
        self.output(self.eof_code);
        self.compressed.clone()
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
        self.clear_hash(H_SIZE);
        self.free_ent = self.clear_code + 2;
        self.clear_flg = true;

        self.output(self.clear_code);
    }

    fn clear_hash(&mut self, h_size: usize) {
        for i in 0..h_size {
            //self.h_tab[i] = -1;
            self.h_tab[i] = 0;
            self.code_tab[i] = 0;
        }
    }

    fn char_out(&mut self, c: u8) {
        self.accum[self.accum_count] = c;
        self.accum_count += 1;
        if self.accum_count >= 254 {
            self.flush_char();
        }
    }

    fn flush_char(&mut self) {
        if self.accum_count > 0 {
            self.write_byte(self.accum_count as u8);
            self.write_sz(
                &self
                    .accum
                    .iter()
                    .take(self.accum_count)
                    .cloned()
                    .collect::<Vec<u8>>(),
                self.accum_count,
            );
            self.accum_count = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_lzw_writer_minimal() {
        let writer = std::io::stdout();
        let bits = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let width = 2;
        let height = 2;
        let bytes_per_pixel = 4;
        let mut lzw_writer = LzwWriter::new(bits, width, height, bytes_per_pixel);
        let compressed = lzw_writer.compress_bits(8 + 1);
        assert_eq!(
            compressed,
            vec![21, 0, 1, 4, 16, 48, 128, 64, 1, 3, 7, 16, 36, 80, 176, 128, 65, 3, 7, 15, 2, 2,]
        );
    }
}
