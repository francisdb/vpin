//! The flat form of a finding and where in the table it points.

use super::references::item_label;
use super::{Kind, NameKind, Severity, VPX};
use std::collections::HashMap;
use std::fmt;

/// A place in the table script, counted the way editors do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub struct ScriptLocation {
    /// Line number, from 1
    pub line: usize,
    /// Column of the first character, from 1, counted in characters, when
    /// known
    pub column: Option<usize>,
}

impl fmt::Display for ScriptLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.column {
            Some(column) => write!(f, "script line {}, column {column}", self.line),
            None => write!(f, "script line {}", self.line),
        }
    }
}

/// A single consistency problem found by [`audit`](super::audit).
///
/// A finding is a severity, a stable code and a message meant to be shown
/// as is, plus the game item and the place in the script it is about when
/// it has one. The code names the check in kebab case, `missing-image` for
/// instance, and is what to group or suppress findings by. New checks add
/// codes; they do not change this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    severity: Severity,
    code: &'static str,
    message: String,
    item: Option<String>,
    location: Option<ScriptLocation>,
}

impl Finding {
    /// How serious this finding is
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// The check that produced this finding, in kebab case, such as
    /// `missing-image` or `timer-without-handler`
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// What is wrong, in one sentence, without the location
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Name of the game item the finding is about, as the table spells it,
    /// for findings about one game item that exists. This is what to select
    /// or jump to; the message already names the item. When several items
    /// share the name it stands for the first one
    pub fn item(&self) -> Option<&str> {
        self.item.as_deref()
    }

    /// Where in the script the finding is, for findings about the script
    /// that point at one place
    pub fn location(&self) -> Option<ScriptLocation> {
        self.location
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location {
            Some(location) => write!(f, "{location}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl Kind {
    /// The flat form of this finding, with the game item and the place in
    /// the script it is about when it has one
    pub(super) fn finding(self, vpx: &VPX, items: &ItemIndex) -> Finding {
        let location = self.locate(&vpx.gamedata.code.string, &vpx.gamedata.name);
        Finding {
            severity: self.severity(),
            code: self.code(),
            message: self.to_string(),
            item: self.item_ref().and_then(|item| items.resolve(item)),
            location,
        }
    }

    /// The game item this finding is about, for the findings about one.
    /// A missing collection item names an item that does not exist and
    /// carries none.
    fn item_ref(&self) -> Option<ItemRef<'_>> {
        match self {
            Kind::MissingImage { item, .. }
            | Kind::MissingMaterial { item, .. }
            | Kind::MissingSurface { item, .. }
            | Kind::MissingPartGroup { item, .. }
            | Kind::NameTooLong { item, .. }
            | Kind::UnstorableText { item, .. }
            | Kind::NonStandardFont { item, .. }
            | Kind::HugeMesh { item, .. }
            | Kind::TextboxUsedForDmd { item }
            | Kind::FastTimer { item, .. }
            | Kind::NegativeLightIntensity { item }
            | Kind::OpaquePrimitiveTranslucency { item }
            | Kind::StaticPrimitiveInScript { item, .. }
            | Kind::LightCannotFade { item, .. } => Some(ItemRef::Label(item)),
            Kind::TimerWithoutHandler { item, .. } => Some(ItemRef::Name(item)),
            Kind::DuplicateName {
                kind: NameKind::GameItem,
                name,
                ..
            }
            | Kind::ReservedName {
                kind: NameKind::GameItem,
                name,
                ..
            }
            | Kind::ScriptNameShadowsItem {
                kind: NameKind::GameItem,
                name,
                ..
            } => Some(ItemRef::Name(name)),
            _ => None,
        }
    }

    /// Where in the script this finding is about, for the findings that
    /// point at one place. Findings that list several names carry none.
    fn locate(&self, script: &str, table_name: &str) -> Option<ScriptLocation> {
        match self {
            Kind::ScriptParseError { location, .. } => *location,
            Kind::MixedScriptLineEndings { .. } => line_ending_change(script),
            Kind::ExecuteUsed => find_word(script, "execute"),
            Kind::RndWithoutRandomize => find_word(script, "rnd"),
            Kind::MissingPinMameTimer | Kind::MissingVpmInit => {
                find_word(script, "loadvpm").or_else(|| find_word(script, "loadvpmalt"))
            }
            Kind::MissingPulseTimer => find_word(script, "vpmtimer"),
            // the parser knows where these names are declared
            Kind::DuplicateProcedure { location, .. }
            | Kind::ScriptNameShadowsItem { location, .. }
            | Kind::UnusedVariable { location, .. }
            | Kind::UnusedLocalVariable { location, .. }
            | Kind::HandlerWithoutItem { location, .. }
            | Kind::BallIdAssigned { location } => Some(*location),
            Kind::StaticPrimitiveInScript { name, .. } => find_word(script, name),
            Kind::DeprecatedTableProperty { property } => {
                find_word(script, &format!("{table_name}.{property}"))
            }
            Kind::DeprecatedControllerProperty { property } => {
                find_word(script, &format!(".{property}"))
            }
            Kind::MissingSound { sound } => script_matches(script, &format!("\"{sound}\""), false)
                .into_iter()
                .next()
                .or_else(|| {
                    script_matches(script, &format!("\"{sound}"), false)
                        .into_iter()
                        .next()
                }),
            _ => None,
        }
    }
}

/// How a [`Kind`] names the game item it is about
enum ItemRef<'a> {
    /// Type and name, as [`item_label`] writes them; a label that is no
    /// game item, `table settings` or a collection, resolves to nothing
    Label(&'a str),
    /// The bare name, in any case
    Name(&'a str),
}

/// The named game items of a table by label and by lower cased name, to
/// resolve the item a finding is about. The first item wins a shared name,
/// like vpinball's lookups.
pub(super) struct ItemIndex<'a> {
    by_label: HashMap<String, &'a str>,
    by_name: HashMap<String, &'a str>,
}

impl<'a> ItemIndex<'a> {
    pub(super) fn new(vpx: &'a VPX) -> Self {
        let mut by_label = HashMap::new();
        let mut by_name = HashMap::new();
        for item in vpx.gameitems.iter().filter(|item| !item.name().is_empty()) {
            by_label.entry(item_label(item)).or_insert(item.name());
            by_name
                .entry(item.name().to_lowercase())
                .or_insert(item.name());
        }
        ItemIndex { by_label, by_name }
    }

    fn resolve(&self, item: ItemRef) -> Option<String> {
        match item {
            ItemRef::Label(label) => self.by_label.get(label),
            ItemRef::Name(name) => self.by_name.get(name.to_lowercase().as_str()),
        }
        .map(|name| name.to_string())
    }
}

/// The first line whose ending differs from the one the script starts with
fn line_ending_change(script: &str) -> Option<ScriptLocation> {
    let bytes = script.as_bytes();
    let mut first: Option<&str> = None;
    let mut line = 1;
    let mut i = 0;
    while i < bytes.len() {
        let ending = match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                i += 1;
                "\r\n"
            }
            b'\r' => "\r",
            b'\n' => "\n",
            _ => {
                i += 1;
                continue;
            }
        };
        match first {
            None => first = Some(ending),
            Some(f) if f != ending => {
                return Some(ScriptLocation { line, column: None });
            }
            Some(_) => {}
        }
        line += 1;
        i += 1;
    }
    None
}

/// Where a word first occurs in the code of the script, comments and
/// string literals excluded, on identifier boundaries
fn find_word(script: &str, word: &str) -> Option<ScriptLocation> {
    script_matches(script, word, true).into_iter().next()
}

/// Every place `needle` occurs in the script, case insensitively, comments
/// excluded. With `in_code` the string literals are blanked so only code
/// matches, otherwise the literal text is searched too. A match must stand
/// on an identifier boundary on each side where the needle itself is an
/// identifier character.
fn script_matches(script: &str, needle: &str, in_code: bool) -> Vec<ScriptLocation> {
    let needle: Vec<char> = needle.to_lowercase().chars().collect();
    let Some((&first, &last)) = needle.first().zip(needle.last()) else {
        return Vec::new();
    };
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    let mut found = Vec::new();
    for (index, line) in script.lines().enumerate() {
        let haystack = searchable_line(line, in_code);
        if haystack.len() < needle.len() {
            continue;
        }
        for start in 0..=haystack.len() - needle.len() {
            let end = start + needle.len();
            if haystack[start..end] != needle[..] {
                continue;
            }
            let bounded_before = !is_ident(first) || start == 0 || !is_ident(haystack[start - 1]);
            let bounded_after =
                !is_ident(last) || end == haystack.len() || !is_ident(haystack[end]);
            if bounded_before && bounded_after {
                found.push(ScriptLocation {
                    line: index + 1,
                    column: Some(start + 1),
                });
            }
        }
    }
    found
}

/// A script line lower cased with its comment cut off and, with
/// `in_code`, its string literals blanked, keeping every character in
/// place so columns still count
fn searchable_line(line: &str, in_code: bool) -> Vec<char> {
    let mut out = Vec::with_capacity(line.len());
    let mut in_string = false;
    for c in line.chars() {
        match (in_string, c) {
            (false, '"') => {
                in_string = true;
                out.push('"');
            }
            (false, '\'') => break,
            (false, c) => out.push(c.to_ascii_lowercase()),
            (true, '"') => {
                in_string = false;
                out.push('"');
            }
            (true, c) => out.push(if in_code { ' ' } else { c.to_ascii_lowercase() }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::GameItemEnum;
    use pretty_assertions::assert_eq;

    fn at(line: usize, column: usize) -> Option<ScriptLocation> {
        Some(ScriptLocation {
            line,
            column: Some(column),
        })
    }

    #[test]
    fn a_finding_is_a_severity_a_code_a_message_and_a_location() {
        let mut vpx = VPX::default();
        vpx.gamedata.code.string = "Option Explicit\r\nDim x\r\nx = Rnd\r\n".to_string();
        let items = ItemIndex::new(&vpx);
        let finding = Kind::RndWithoutRandomize.finding(&vpx, &items);
        assert_eq!(finding.severity(), Severity::Suggestion);
        assert_eq!(finding.code(), "rnd-without-randomize");
        assert_eq!(
            finding.message(),
            "script uses Rnd without Randomize, so every run draws the same numbers"
        );
        assert_eq!(finding.location(), at(3, 5));
        assert_eq!(
            finding.to_string(),
            "script line 3, column 5: script uses Rnd without Randomize, so every run draws the same numbers"
        );

        let finding = Kind::MissingTableName.finding(&vpx, &items);
        assert_eq!(finding.code(), "missing-table-name");
        assert_eq!(finding.location(), None);
        assert_eq!(finding.to_string(), "table info has no table name");
    }

    #[test]
    fn a_finding_about_a_game_item_names_it() {
        let mut vpx = VPX::default();
        for name in ["Apron", "apron"] {
            vpx.add_game_item(GameItemEnum::Wall(crate::vpx::gameitem::wall::Wall {
                name: name.to_string(),
                ..Default::default()
            }));
        }
        let items = ItemIndex::new(&vpx);
        let item = |kind: Kind| kind.finding(&vpx, &items).item().map(str::to_string);

        let labelled = Kind::MissingSurface {
            item: "Wall \"Apron\"".to_string(),
            surface: "Gone".to_string(),
        };
        assert_eq!(item(labelled), Some("Apron".to_string()));
        // a bare name resolves case insensitively, to the first item
        let named = Kind::TimerWithoutHandler {
            item: "APRON".to_string(),
            interval: 100,
        };
        assert_eq!(item(named), Some("Apron".to_string()));
        let duplicate = Kind::DuplicateName {
            kind: NameKind::GameItem,
            name: "Apron".to_string(),
            count: 2,
        };
        assert_eq!(item(duplicate), Some("Apron".to_string()));
    }

    #[test]
    fn a_finding_about_anything_else_names_no_item() {
        let mut vpx = VPX::default();
        vpx.add_game_item(GameItemEnum::Wall(crate::vpx::gameitem::wall::Wall {
            name: "Apron".to_string(),
            ..Default::default()
        }));
        let items = ItemIndex::new(&vpx);
        let item = |kind: Kind| kind.finding(&vpx, &items).item().map(str::to_string);

        // an image that shares its name with a game item
        let image = Kind::BmpImage {
            image: "Apron".to_string(),
        };
        assert_eq!(item(image), None);
        let table = Kind::MissingMaterial {
            item: "table settings".to_string(),
            field: "playfield material",
            material: "Gone".to_string(),
        };
        assert_eq!(item(table), None);
        let collection = Kind::NameTooLong {
            item: "Collection \"Apron\"".to_string(),
            length: 40,
        };
        assert_eq!(item(collection), None);
        let missing = Kind::MissingCollectionItem {
            collection: "Targets".to_string(),
            item: "Gone".to_string(),
        };
        assert_eq!(item(missing), None);
    }

    #[test]
    fn codes_are_kebab_case() {
        let codes = [
            Kind::MissingImage {
                item: String::new(),
                field: "",
                image: String::new(),
            }
            .code(),
            Kind::MissingPinMameTimer.code(),
            Kind::TextboxUsedForDmd {
                item: String::new(),
            }
            .code(),
            Kind::ColorGradeLutUnusualSize {
                image: String::new(),
                width: 0,
                height: 0,
            }
            .code(),
        ];
        assert_eq!(
            codes,
            [
                "missing-image",
                "missing-pinmame-timer",
                "textbox-used-for-dmd",
                "color-grade-lut-unusual-size"
            ]
        );
        for code in codes {
            assert!(
                code.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{code}"
            );
        }
    }

    #[test]
    fn words_are_found_in_code_but_not_in_comments_or_strings() {
        let script = "' Execute here is a comment\r\nx = \"Execute\"\r\n  ExecuteGlobal s\r\n  Execute s\r\n";
        assert_eq!(find_word(script, "execute"), at(4, 3));
        assert_eq!(find_word(script, "executeglobal"), at(3, 3));
        assert_eq!(find_word(script, "missing"), None);
    }

    #[test]
    fn member_and_qualified_names_are_bounded_on_the_identifier_side_only() {
        let script = "With Controller\r\n  .ShowFrame = False\r\n  .ShowFrameRate = 1\r\nEnd With\r\nTable1.Inclination = 5\r\n";
        assert_eq!(find_word(script, ".showframe"), at(2, 3));
        assert_eq!(find_word(script, "table1.inclination"), at(5, 1));
        // a member name stands on a boundary after its dot
        assert_eq!(find_word(script, "inclination"), at(5, 8));
        assert_eq!(find_word(script, "clination"), None);
    }

    #[test]
    fn a_missing_sound_is_located_at_its_literal() {
        let script = "PlaySound \"fx_flip\"\r\nPlaySound \"fx_ball\" & i\r\n";
        let located = |sound: &str| {
            Kind::MissingSound {
                sound: sound.to_string(),
            }
            .locate(script, "")
        };
        assert_eq!(located("fx_flip"), at(1, 11));
        assert_eq!(located("FX_BALL"), at(2, 11));
        assert_eq!(located("fx_other"), None);
    }

    #[test]
    fn a_static_primitive_is_located_at_its_first_mention() {
        let script = "Dim Apron\r\nBaked.Visible = False\r\n";
        let kind = Kind::StaticPrimitiveInScript {
            name: "Baked".to_string(),
            item: "Primitive \"Baked\"".to_string(),
            script_toggles_prerendering: false,
        };
        assert_eq!(kind.locate(script, ""), at(2, 1));
    }

    #[test]
    fn the_vpinmame_findings_are_located_at_the_load_call() {
        let script = "LoadVPM \"01560000\", \"sys80.vbs\", 3.1\r\nvpmTimer.PulseSw 1\r\n";
        assert_eq!(Kind::MissingPinMameTimer.locate(script, ""), at(1, 1));
        assert_eq!(Kind::MissingVpmInit.locate(script, ""), at(1, 1));
        assert_eq!(Kind::MissingPulseTimer.locate(script, ""), at(2, 1));
    }

    #[test]
    fn a_parse_error_carries_the_location_the_parser_gave() {
        let kind = Kind::ScriptParseError {
            detail: "unexpected token".to_string(),
            location: at(12, 4),
        };
        assert_eq!(kind.locate("", ""), at(12, 4));
        assert_eq!(
            kind.to_string(),
            "script could not be parsed: unexpected token"
        );
        let kind = Kind::ScriptParseError {
            detail: "unexpected token".to_string(),
            location: None,
        };
        assert_eq!(kind.locate("", ""), None);
    }

    #[test]
    fn mixed_line_endings_are_located_at_the_first_change() {
        let script = "Option Explicit\r\nRandomize\r\nRandomize\nRandomize\r\n";
        let kind = Kind::MixedScriptLineEndings {
            crlf: 3,
            lf: 1,
            cr: 0,
        };
        assert_eq!(
            kind.locate(script, ""),
            Some(ScriptLocation {
                line: 3,
                column: None
            })
        );
        assert_eq!(line_ending_change("a\r\nb\r\n"), None);
    }

    #[test]
    fn deprecated_properties_are_located_at_their_access() {
        let script =
            "' Table1.Inclination\r\nTable1.Inclination = 5\r\nController.ShowFrame = 0\r\n";
        let kind = Kind::DeprecatedTableProperty {
            property: "Inclination".to_string(),
        };
        assert_eq!(kind.locate(script, "Table1"), at(2, 1));
        let kind = Kind::DeprecatedControllerProperty {
            property: "ShowFrame".to_string(),
        };
        assert_eq!(kind.locate(script, "Table1"), at(3, 11));
    }

    #[test]
    fn a_finding_about_a_declared_name_carries_its_location() {
        // the parser found it, the script is not searched for the name
        let kind = Kind::HandlerWithoutItem {
            name: "Gone_Hit".to_string(),
            location: ScriptLocation {
                line: 7,
                column: Some(5),
            },
        };
        assert_eq!(kind.locate("", ""), at(7, 5));
    }
}
