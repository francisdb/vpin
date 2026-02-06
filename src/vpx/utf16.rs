use std::io;

pub(crate) fn decode_utf16le(bytes: &[u8]) -> io::Result<String> {
    if !bytes.len().is_multiple_of(2) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Input byte slice length must be even for UTF-16LE decoding",
        ));
    }

    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    String::from_utf16(&units).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

pub(crate) fn encode_utf16le(value: &str) -> Vec<u8> {
    value
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf16le_encoding_decoding() {
        let original = "Hello, 世界!";
        let encoded = encode_utf16le(original);
        let decoded = decode_utf16le(&encoded).expect("Decoding failed");
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_utf16le_decoding_invalid_length() {
        let invalid_bytes = vec![0x00, 0x00, 0x00]; // Odd length
        let result = decode_utf16le(&invalid_bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_utf16le_decoding_invalid_data() {
        let invalid_cases = [
            vec![0x00, 0xD8],             // lone high surrogate (0xD800)
            vec![0x00, 0xDC],             // lone low surrogate (0xDC00)
            vec![0x00, 0xD8, 0x41, 0x00], // high surrogate followed by non-low surrogate
            vec![0x00, 0xDC, 0x00, 0xD8], // low surrogate followed by high surrogate
        ];

        for invalid_bytes in invalid_cases {
            let result = decode_utf16le(&invalid_bytes);
            assert!(result.is_err());
        }
    }
}
