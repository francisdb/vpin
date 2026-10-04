//! Mapping entity names to pack file names

use std::collections::HashSet;

/// The file name vpinball gives an entity, without extension.
///
/// Characters invalid on common file systems (`/ \ : * ? " < > |`) and
/// control characters become `_`, trailing dots and spaces are stripped,
/// and an empty result, `.` or `..` becomes `_`.
///
/// vpinball tests each byte of the UTF-8 name as a signed `char`, so on
/// the platforms it ships for every byte of a non-ASCII character becomes
/// `_` as well: `Mélo` maps to `M__lo`. This function does the same, so
/// packs it writes resolve the same way in vpinball.
pub fn sanitize_file_name(name: &str) -> String {
    let mut result: String = name
        .bytes()
        .map(|b| {
            if !(0x20..0x80).contains(&b) || b"/\\:*?\"<>|".contains(&b) {
                '_'
            } else {
                b as char
            }
        })
        .collect();
    trim_end_dots_and_spaces(&mut result);
    if result.is_empty() || result == "." || result == ".." {
        result = "_".to_string();
    }
    result
}

/// [`sanitize_file_name`] that keeps non-ASCII characters, as vpinball
/// does on platforms where `char` is unsigned
pub(super) fn sanitize_file_name_utf8(name: &str) -> String {
    let mut result: String = name
        .chars()
        .map(|c| {
            if (c as u32) < 0x20 || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    trim_end_dots_and_spaces(&mut result);
    if result.is_empty() || result == "." || result == ".." {
        result = "_".to_string();
    }
    result
}

fn trim_end_dots_and_spaces(name: &mut String) {
    let trimmed = name.trim_end_matches(['.', ' ']).len();
    name.truncate(trimmed);
}

/// Hands out file stems unique within one pack folder, ignoring ASCII
/// case so the pack also works on case insensitive file systems.
/// Collisions get a `_2`, `_3`, ... suffix, as in vpinball.
#[derive(Default)]
pub(super) struct StemPool {
    used: HashSet<String>,
}

impl StemPool {
    pub(super) fn unique(&mut self, name: &str) -> String {
        let base = sanitize_file_name(name);
        let mut stem = base.clone();
        let mut index = 2;
        while !self.used.insert(stem.to_ascii_lowercase()) {
            stem = format!("{base}_{index}");
            index += 1;
        }
        stem
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn invalid_characters_become_underscores() {
        assert_eq!(
            sanitize_file_name("a/b\\c:d*e?f\"g<h>i|j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
        assert_eq!(sanitize_file_name("tab\there"), "tab_here");
    }

    #[test]
    fn non_ascii_bytes_become_underscores() {
        assert_eq!(sanitize_file_name("Mélo"), "M__lo");
        assert_eq!(sanitize_file_name_utf8("Mélo"), "Mélo");
    }

    #[test]
    fn trailing_dots_and_spaces_are_stripped() {
        assert_eq!(sanitize_file_name("name. ."), "name");
        assert_eq!(sanitize_file_name(" lead"), " lead");
    }

    #[test]
    fn empty_and_dot_names_become_an_underscore() {
        assert_eq!(sanitize_file_name(""), "_");
        assert_eq!(sanitize_file_name("..."), "_");
        assert_eq!(sanitize_file_name("  "), "_");
    }

    #[test]
    fn collisions_get_a_suffix_ignoring_case() {
        let mut pool = StemPool::default();
        assert_eq!(pool.unique("Wall"), "Wall");
        assert_eq!(pool.unique("wall"), "wall_2");
        assert_eq!(pool.unique("a/b"), "a_b");
        assert_eq!(pool.unique("a_b"), "a_b_2");
        assert_eq!(pool.unique("WALL"), "WALL_3");
    }
}
