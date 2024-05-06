//! Port of VisualPinballEngine's LZW decompression algorithm.
//! which is a port of VisualPinball's LZW decompression algorithm.
//!
//! https://github.com/vpinball/vpinball/blob/master/media/lzwreader.h
//! https://github.com/vpinball/vpinball/blob/master/media/lzwreader.cpp
//!
//! https://github.com/freezy/VisualPinball.Engine/blob/master/VisualPinball.Engine/IO/LzwReader.cs

// DECODE.C - An LZW decoder for GIF
// Copyright (C) 1987, by Steven A. Bennett
//
// Permission is given by the author to freely redistribute and include
// this code in any program as long as this credit is given where due.
//
// In accordance with the above, I want to credit Steve Wilhite who wrote
// the code which this is heavily inspired by...
//
// GIF and 'Graphics Interchange Format' are trademarks (tm) of
// Compuserve, Incorporated, an H&R Block Company.
//
// Release Notes: This file contains a decoder routine for GIF images
// which is similar, structurally, to the original routine by Steve Wilhite.
// It is, however, somewhat noticeably faster in most cases.

use byteorder::ReadBytesExt;
use std::io::Read;

const MAX_CODES: u16 = 4095;
const FILE_BUF_SIZE: i32 = 4096;

const CODE_MASK: [u16; 13] = [
    0, 0x0001, 0x0003, 0x0007, 0x000F, 0x001F, 0x003F, 0x007F, 0x00FF, 0x01FF, 0x03FF, 0x07FF,
    0x0FFF,
];

/// <summary>
/// This is a port of VPinball's lzwreader which is used to decompress
/// bitmaps.
/// </summary>
/// <see href="https://github.com/vpinball/vpinball/blob/master/media/lzwreader.cpp"/>
pub(crate) struct LzwReader {
    /* input */
    compressed_data: Box<dyn Read>,

    /* output */
    decompressed_data: Vec<u8>,
    decompressed_data_cursor: usize,

    // pitch/stride refers to the number of bytes in a row of pixel data in memory
    stride: u32,

    bad_code_count: u32,

    /* Static variables */
    /// The current code size (max 12 bits)
    current_code_size: u8,
    /// Value for a clear code
    clear_code: u16,
    /// Value for a ending code
    ending_code: u16,
    /// First available code
    new_codes: u16,
    /// Highest code for current size
    top_slot: u16,
    /// Last read code
    slot: u16,

    /* The following static variables are used
     * for separating out codes
     */
    /// bytes left in block
    num_avail_bytes: u8,
    /// bits left in current byte
    num_bits_left: u8,
    /// Current byte
    b1: u8,
    /// Current block
    block_buff: [u8; 257],
    /// points to byte_buff - Pointer to next byte in block
    block_buff_cursor: usize,

    /// Stack for storing pixels
    stack: [u8; MAX_CODES as usize + 1],
    /// Suffix table
    suffix: [u8; MAX_CODES as usize + 1],
    /// Prefix linked list
    prefix: [u16; MAX_CODES as usize + 1],

    /// same as stride?
    width: u32,
    // height: u32,
    lines_left: u32,
}

impl LzwReader {
    /// pstm is the compressed data
    /// width is the number of pixels per line
    /// pitch is the number of bytes per line
    /// height is the number of lines
    pub(crate) fn new<R: Read + 'static>(
        compressed_data: R,
        width: u32,
        height: u32,
        bytes_per_pixel: u8,
    ) -> LzwReader {
        let stride = width * bytes_per_pixel as u32;
        let bits_out = vec![0; (stride * height) as usize];
        let pb_bits_out_cur: usize = 0;

        LzwReader {
            compressed_data: Box::new(compressed_data),
            decompressed_data: bits_out,
            decompressed_data_cursor: pb_bits_out_cur,
            stride,
            bad_code_count: 0,
            current_code_size: 0,
            clear_code: 0,
            ending_code: 0,
            new_codes: 0,
            top_slot: 0,
            slot: 0,
            num_avail_bytes: 0,
            num_bits_left: 0,
            b1: 0,
            block_buff: [0; 257],
            block_buff_cursor: 0,
            stack: [0; MAX_CODES as usize + 1],
            suffix: [0; MAX_CODES as usize + 1],
            prefix: [0; MAX_CODES as usize + 1],
            // strange that is the same as stride
            width: stride,
            // height,
            lines_left: height + 1,
        }
    }

    pub(crate) fn decompress(&mut self) -> Vec<u8> {
        let mut fc: u16 = 0;

        // Initialize for decoding a new image...
        let code_size: u8 = 8;
        self.init_exp(code_size);

        // Initialize in case they forgot to put in a clear code.
        // (This shouldn't happen, but we'll try and decode it anyway...)
        let mut oc = fc;

        // Allocate space for the decode buffer
        let mut buf = self.next_line();

        // Set up the stack pointer and decode buffer pointer
        let mut sp = 0;
        let mut buf_ptr = buf.clone();
        let mut buf_cnt = self.width;

        // This is the main loop.  For each code we get we pass through the
        // linked list of prefix codes, pushing the corresponding "character" for
        // each code onto the stack.  When the list reaches a single "character"
        // we push that on the stack too, and then start unstacking each
        // character for output in the correct order.  Special handling is
        // included for the clear code, and the whole thing ends when we get
        // an ending code.
        let mut c = self.get_next_code();
        while c != self.ending_code {
            // If the code is a clear code, reinitialize all necessary items.
            if c == self.clear_code {
                self.current_code_size = code_size + 1;
                self.slot = self.new_codes;
                self.top_slot = 1 << self.current_code_size;

                // Continue reading codes until we get a non-clear code
                // (Another unlikely, but possible case...)
                c = self.get_next_code();
                while c == self.clear_code {
                    c = self.get_next_code();
                }

                // If we get an ending code immediately after a clear code
                // (Yet another unlikely case), then break out of the loop.
                if c == self.ending_code {
                    break;
                }

                // Finally, if the code is beyond the range of already set codes,
                // (This one had better NOT happen...  I have no idea what will
                // result from this, but I doubt it will look good...) then set it
                // to color zero.
                if c >= self.slot {
                    c = 0;
                }

                fc = c;
                oc = c;

                // And let us not forget to put the char into the buffer... And
                // if, on the off chance, we were exactly one pixel from the end
                // of the line, we have to send the buffer to the out_line()
                // routine...
                self.decompressed_data[buf_ptr] = c as u8;
                buf_ptr += 1;
                // buf_ptr.set(c as u8);
                // buf_ptr.incr();

                if buf_cnt == 0 {
                    buf = self.next_line();
                    buf_ptr = buf.clone();
                    buf_cnt = self.width;
                }
            } else {
                // In this case, it's not a clear code or an ending code, so
                // it must be a code code...  So we can now decode the code into
                // a stack of character codes. (Clear as mud, right?)
                let mut code = c;

                // Here we go again with one of those off chances...  If, on the
                // off chance, the code we got is beyond the range of those already
                // set up (Another thing which had better NOT happen...) we trick
                // the decoder into thinking it actually got the last code read.
                // (Hmmn... I'm not sure why this works...  But it does...)
                if code >= self.slot {
                    if code > self.slot {
                        self.bad_code_count += 1;
                    }
                    code = oc;
                    self.stack[sp] = fc as u8;
                    sp += 1;
                    // sp.set(fc as u8);
                    // sp.incr();
                }

                // Here we scan back along the linked list of prefixes, pushing
                // helpless characters (ie. suffixes) onto the stack as we do so.
                while code >= self.new_codes {
                    self.stack[sp] = self.suffix[code as usize];
                    sp += 1;
                    // sp.set(self.suffix[code as usize]);
                    // sp.incr();
                    code = self.prefix[code as usize];
                }

                // Push the last character on the stack, and set up the new
                // prefix and suffix, and if the required slot number is greater
                // than that allowed by the current bit size, increase the bit
                // size.  (NOTE - If we are all full, we *don't* save the new
                // suffix and prefix...  I'm not certain if this is correct...
                // it might be more proper to overwrite the last code...
                self.stack[sp] = code as u8;
                sp += 1;
                //sp.set(code as u8);
                //sp.incr();
                if self.slot < self.top_slot {
                    fc = code;
                    self.suffix[self.slot as usize] = fc as u8;
                    self.prefix[self.slot as usize] = oc;
                    self.slot += 1;
                    oc = c;
                }
                if self.slot >= self.top_slot {
                    if self.current_code_size < 12 {
                        self.top_slot <<= 1;
                        self.current_code_size += 1;
                    }
                }

                // Now that we've pushed the decoded string (in reverse order)
                // onto the stack, lets pop it off and put it into our decode
                // buffer...  And when the decode buffer is full, write another
                // line...
                while sp > 0 {
                    sp -= 1;
                    //sp.decr();
                    self.decompressed_data[buf_ptr] = self.stack[sp];
                    buf_ptr += 1;
                    // buf_ptr.set(sp.get());
                    // buf_ptr.incr();
                    if buf_cnt == 0 {
                        buf = self.next_line();
                        buf_ptr = buf.clone();
                        buf_cnt = self.width;
                    }
                }
            }
            c = self.get_next_code();
        }

        // Length of the compressed data
        // We currently don't use this as we have a hacky way of getting the length
        // by checking for a ALTV biff tag.
        // let length = self.pstm.get_pos();

        self.decompressed_data.clone()
    }

    /// This function initializes the decoder for reading a new image.
    fn init_exp(&mut self, size: u8) {
        self.current_code_size = size + 1;
        self.top_slot = 1 << self.current_code_size;
        self.clear_code = 1 << size;
        self.ending_code = self.clear_code + 1;

        self.new_codes = self.ending_code + 1;
        self.slot = self.new_codes;

        self.num_bits_left = 0;
        self.num_avail_bytes = 0;
    }

    fn next_line(&mut self) -> usize {
        let next_line_cursor = self.decompressed_data_cursor;
        self.decompressed_data_cursor += self.stride as usize; // fucking upside down dibs!
        self.lines_left -= 1;
        return next_line_cursor;
    }

    /// gets the next code from the GIF file.  Returns the code, or else
    /// a negative number in case of file errors...
    fn get_next_code(&mut self) -> u16 {
        let mut ret: u16;
        if self.num_bits_left == 0 {
            self.read_next_block();
            self.b1 = self.block_buff[self.block_buff_cursor];
            self.block_buff_cursor += 1;
            self.num_bits_left = 8;
            if self.num_avail_bytes == 0 {
                panic!("LZW decode failed, expected more bytes while reading next code.")
            }
            self.num_avail_bytes -= 1;
        }

        ret = self.b1 as u16 >> (8 - self.num_bits_left);
        while self.current_code_size > self.num_bits_left {
            self.read_next_block();
            self.b1 = self.block_buff[self.block_buff_cursor];
            self.block_buff_cursor += 1;
            ret |= (self.b1 as u16) << self.num_bits_left;
            self.num_bits_left += 8;
            if self.num_avail_bytes == 0 {
                panic!("LZW decode failed, expected more bytes while reading next code.")
            }
            self.num_avail_bytes -= 1;
        }
        self.num_bits_left -= self.current_code_size;
        ret &= CODE_MASK[self.current_code_size as usize];
        ret
    }

    fn read_next_block(&mut self) {
        if self.num_avail_bytes <= 0 {
            // Out of bytes in current block, so read next block
            self.block_buff_cursor = 0;
            self.num_avail_bytes = self.get_byte();

            if self.num_avail_bytes > 0 {
                for i in 0..self.num_avail_bytes as usize {
                    let x = self.get_byte();
                    self.block_buff[i] = x;
                }
            }
        }
    }

    fn get_byte(&mut self) -> u8 {
        self.compressed_data.read_u8().unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::io::Cursor;

    const LZW_COMPRESSED_DATA: [u8; 14] =
        [13, 0, 255, 169, 82, 37, 176, 224, 192, 127, 8, 19, 6, 4];

    #[test]
    fn test_lzw_reader_minimal() {
        let compressed_data: Vec<u8> =
            vec![13, 0, 255, 169, 82, 37, 176, 224, 192, 127, 8, 19, 6, 4];
        let compressed_data = Cursor::new(compressed_data);
        let mut lzw_reader = LzwReader::new(Box::new(compressed_data), 2, 2, 4);
        let decompressed_data = lzw_reader.decompress();
        #[rustfmt::skip]
        let expected = vec![
            0xFF, 0xAA, 0xAA, 0xFF, // red
            0xAA, 0xFF, 0xAA, 0xFF, // green
            0xAA, 0xAA, 0xFF, 0xFF, // blue
            0xFF, 0xFF, 0xFF, 0xFF // white
        ];
        assert_eq!(decompressed_data, expected);
    }

    #[test]
    fn test_lzw_reader_real_file() {
        let file = "testdata/raw_lzw_bmp_128_128_data.bin";
        let file = std::fs::File::open(file).unwrap();
        // read all the bytes
        let data = std::io::BufReader::new(file)
            .bytes()
            .map(|b| b.unwrap())
            .collect::<Vec<u8>>();
        let compressed_data = Cursor::new(data);

        // Stored in vpx file as compressed ARGB data
        let width = 128;
        let height = 128;
        let bytes_per_pixel = 4;

        // data was written like this:
        // var lzwWriter = new LzwWriter(writer, ToggleRgbBgr(Data), Width * 4, Height, Pitch());
        // lzwWriter.CompressBits(8 + 1);
        //
        // And read like this:
        // // BMP stored as a 32-bit SBGRA picture
        // BYTE* const __restrict tmp = new BYTE[(size_t)m_width * m_height * 4];
        // LZWReader lzwreader(pbr->m_pistream, (int *)tmp, m_width * 4, m_height, m_width * 4);
        // lzwreader.Decoder();

        let mut lzw_reader =
            LzwReader::new(Box::new(compressed_data), width, height, bytes_per_pixel);
        let decompressed_data = lzw_reader.decompress();
        assert_eq!(decompressed_data.len(), (width * height * 4) as usize);
        // first 10 bytes
        assert_eq!(
            &decompressed_data[0..10],
            [107, 109, 112, 255, 104, 107, 109, 255, 103, 104,]
        );
        // last 10 bytes
        assert_eq!(
            &decompressed_data[decompressed_data.len() - 10..],
            [177, 255, 179, 173, 181, 255, 182, 177, 185, 255,]
        );
    }
}
