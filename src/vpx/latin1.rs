use crate::vpx::model::encode_latin1_lossy;
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::{Borrow, Cow};
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

/// A string a Latin-1 record can carry unchanged.
///
/// Most text in a table (image, material, surface, sound and layer names)
/// is stored as Latin-1, one byte per character and read up to the first
/// NUL, so the only characters such a field can hold are `U+0001` to
/// `U+00FF`. This type makes that part of the API: the fallible conversions
/// reject anything else, and [`Latin1String::from_lossy`] is the explicit way
/// to replace it with `?` like vpinball does on save.
///
/// It dereferences to [`str`] and serializes as a plain JSON string;
/// deserializing a string the record cannot carry is an error.
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Latin1String(String);

/// The first character of a string that a Latin-1 record cannot carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Latin1Error {
    /// The offending character.
    pub character: char,
    /// Its byte offset in the string.
    pub index: usize,
}

impl fmt::Display for Latin1Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "character {:?} at byte {} cannot be stored as Latin-1",
            self.character, self.index
        )
    }
}

impl std::error::Error for Latin1Error {}

fn is_latin1(c: char) -> bool {
    matches!(u32::from(c), 0x01..=0xFF)
}

fn validate(s: &str) -> Result<(), Latin1Error> {
    if s.is_ascii() && !s.as_bytes().contains(&0) {
        return Ok(());
    }
    match s.char_indices().find(|(_, c)| !is_latin1(*c)) {
        Some((index, character)) => Err(Latin1Error { character, index }),
        None => Ok(()),
    }
}

impl Latin1String {
    /// An empty string.
    pub const fn new() -> Self {
        Self(String::new())
    }

    /// Converts a string, replacing every character a Latin-1 record cannot
    /// carry with `?`.
    pub fn from_lossy(s: &str) -> Self {
        match validate(s) {
            Ok(()) => Self(s.to_string()),
            Err(_) => Self(
                s.chars()
                    .map(|c| if is_latin1(c) { c } else { '?' })
                    .collect(),
            ),
        }
    }

    /// Wraps a string decoded from a Latin-1 record, which is valid by
    /// construction.
    pub(crate) fn from_decoded(s: String) -> Self {
        debug_assert!(validate(&s).is_ok());
        Self(s)
    }

    /// The string as a slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Unwraps the underlying [`String`].
    pub fn into_string(self) -> String {
        self.0
    }

    /// The Latin-1 bytes of the string, one per character. Borrows when the
    /// string is ASCII.
    pub fn to_bytes(&self) -> Cow<'_, [u8]> {
        encode_latin1_lossy(&self.0)
    }
}

impl TryFrom<String> for Latin1String {
    type Error = Latin1Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        validate(&s).map(|()| Self(s))
    }
}

impl TryFrom<&str> for Latin1String {
    type Error = Latin1Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        validate(s).map(|()| Self(s.to_string()))
    }
}

impl FromStr for Latin1String {
    type Err = Latin1Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

impl From<Latin1String> for String {
    fn from(s: Latin1String) -> Self {
        s.0
    }
}

impl Deref for Latin1String {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Latin1String {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Latin1String {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Latin1String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for Latin1String {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl PartialEq<str> for Latin1String {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Latin1String {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for Latin1String {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl PartialEq<Latin1String> for str {
    fn eq(&self, other: &Latin1String) -> bool {
        self == other.0
    }
}

impl PartialEq<Latin1String> for &str {
    fn eq(&self, other: &Latin1String) -> bool {
        *self == other.0
    }
}

impl PartialEq<Latin1String> for String {
    fn eq(&self, other: &Latin1String) -> bool {
        self == &other.0
    }
}

impl<'de> Deserialize<'de> for Latin1String {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for Latin1String {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    /// Printable ASCII and the Latin-1 supplement, at most 32 characters.
    fn arbitrary_with((): Self::Parameters) -> Self::Strategy {
        use proptest::strategy::Strategy;
        proptest::string::string_regex("[ -~\u{A0}-\u{FF}]{0,32}")
            .unwrap()
            .prop_map(Latin1String)
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn accepts_what_a_record_can_carry() {
        let s = Latin1String::try_from("M\u{e9}tal \u{ff}").unwrap();
        assert_eq!(s, "M\u{e9}tal \u{ff}");
        assert_eq!(s.to_bytes().as_ref(), b"M\xe9tal \xff");
    }

    #[test]
    fn rejects_the_first_character_outside_latin1() {
        assert_eq!(
            "ok \u{2713} \u{2713}".parse::<Latin1String>(),
            Err(Latin1Error {
                character: '\u{2713}',
                index: 3
            })
        );
        // the reader stops at the first NUL
        assert_eq!(
            Latin1String::try_from("a\0b".to_string()),
            Err(Latin1Error {
                character: '\0',
                index: 1
            })
        );
    }

    #[test]
    fn lossy_conversion_replaces_with_question_marks() {
        assert_eq!(Latin1String::from_lossy("Metal \u{2713} ok"), "Metal ? ok");
        assert_eq!(Latin1String::from_lossy("M\u{e9}tal"), "M\u{e9}tal");
    }

    #[test]
    fn json_is_a_plain_string_and_rejects_on_read() {
        let s = Latin1String::try_from("M\u{e9}tal").unwrap();
        assert_eq!(serde_json::to_value(&s).unwrap(), json!("M\u{e9}tal"));
        assert_eq!(
            serde_json::from_value::<Latin1String>(json!("M\u{e9}tal")).unwrap(),
            s
        );
        let err = serde_json::from_value::<Latin1String>(json!("\u{2713}")).unwrap_err();
        assert!(err.to_string().contains("cannot be stored as Latin-1"));
    }
}
