//! LZW compression and decompression for BMP raw bitmaps stored compressed in vpx files
//!
//! NOTE: Visual Pinball uses its own LZW implementation that differs slightly from the standard LZW
//! implementation. However, Visual Pinball can also read the compressed data we produce.
//!
//! <https://github.com/vpinball/vpinball/blob/master/media/lzwwriter.h>
//! <https://github.com/vpinball/vpinball/blob/master/media/lzwwriter.cpp>
//!
//! <https://github.com/freezy/VisualPinball.Engine/blob/master/VisualPinball.Engine/IO/LzwWriter.cs>
//! <https://github.com/freezy/VisualPinball.Engine/blob/master/VisualPinball.Engine/IO/LzwReader.cs>

use weezl::BitOrder;

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
fn to_blocks(compressed: &[u8], max_block_len: u8) -> Vec<u8> {
    let mut blocks: Vec<u8> = vec![];
    let mut block: Vec<u8> = vec![];
    let mut block_len = 0;
    for byte in compressed {
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
        .encode(data)
        .unwrap()
}

pub fn to_lzw_blocks(data: &[u8]) -> Vec<u8> {
    let compressed = to_lzw(data);
    // convert compressed bytes to gif blocks
    to_blocks(&compressed, 254)
}

pub fn from_lzw_blocks(compressed: &[u8]) -> Vec<u8> {
    // convert gif blocks to compressed bytes
    let compressed = from_blocks(compressed);
    from_lzw(&compressed)
}

fn from_lzw(compressed: &[u8]) -> Vec<u8> {
    weezl::decode::Decoder::new(BitOrder::Lsb, 8)
        .decode(compressed)
        .unwrap()
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
                            "lzw_to_codes - clear code, reset code size to {start_code_width}"
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
        let blocks = to_blocks(&compressed, max_block_len);
        let uncompressed = from_blocks(&blocks);
        assert_eq!(uncompressed, compressed);
    }

    #[test]
    fn test_to_blocks_empty() {
        let compressed = vec![];
        let max_block_len = 3;
        let blocks = to_blocks(&compressed, max_block_len);

        // should this be [0] ?
        assert_eq!(blocks, [0u8; 0]);
    }

    #[test]
    fn test_lzw_writer_four_0() {
        let bits = vec![0; 4];

        let compressed_blocks = to_lzw_blocks(&bits);
        assert_eq!(compressed_blocks, [6, 0, 1, 8, 4, 16, 16]);

        let codes = lzw_blocks_to_codes(&compressed_blocks);
        // 257 = eof code
        // 256 = clear code (reset dictionary)
        // 0 = 0
        // 258 = 0 0
        assert_eq!(codes, vec![256, 0, 258, 0, 257]);
        // which corresponds to 0 00 0 which was the input
    }

    #[test]
    fn test_lzw_writer_four_255() {
        let bits = vec![255; 4];
        let compressed = to_lzw_blocks(&bits);
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
        let compressed = to_lzw_blocks(&bits);
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

        let compressed_blocks = to_lzw_blocks(&bits);
        let compressed = from_blocks(&compressed_blocks);

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
    }

    const RAW_LZW_BMP_128_128_DATA: &[u8] =
        include_bytes!("../../testdata/raw_lzw_bmp_128_128_data.bin");

    #[test]
    fn test_lzw_read_write() {
        let width: u32 = 128;
        let height: u32 = 128;
        let bytes_per_pixel: u8 = 4;
        let compressed_original = RAW_LZW_BMP_128_128_DATA;
        let decompressed = from_lzw_blocks(compressed_original);

        assert_eq!(
            decompressed.len(),
            (width * height * bytes_per_pixel as u32) as usize
        );
    }
}
