//! What images, sounds, game items, collections and materials are called:
//! duplicates, names the script already means something by, names longer
//! than the editor allows, items without a name and a table without one.

use super::references::item_label;
use super::{Kind, NameKind, ReservedName, VPX};
use crate::vpx::gameitem::MAX_NAME_LENGTH;
use std::collections::{HashMap, HashSet};

/// Names too long for the editor, items without a name, names the
/// script already means something by, for game items and collections
pub(super) fn check_names(vpx: &VPX, findings: &mut Vec<Kind>) {
    for item in &vpx.gameitems {
        let name = item.name();
        if name.chars().count() > MAX_NAME_LENGTH {
            findings.push(Kind::NameTooLong {
                item: item_label(item),
                length: name.chars().count(),
            });
        }
    }

    let mut unnamed: Vec<(String, usize)> = Vec::new();
    for item in vpx.gameitems.iter().filter(|item| item.name().is_empty()) {
        let type_name = item.type_name();
        match unnamed.iter_mut().find(|(name, _)| *name == type_name) {
            Some((_, count)) => *count += 1,
            None => unnamed.push((type_name, 1)),
        }
    }
    for (type_name, count) in unnamed {
        findings.push(Kind::UnnamedItems { type_name, count });
    }

    for item in &vpx.gameitems {
        if let Some(reserved) = reserved_name(item.name()) {
            findings.push(Kind::ReservedName {
                kind: NameKind::GameItem,
                name: item.name().to_string(),
                reserved,
            });
        }
    }
    for collection in &vpx.collections {
        if let Some(reserved) = reserved_name(&collection.name) {
            findings.push(Kind::ReservedName {
                kind: NameKind::Collection,
                name: collection.name.clone(),
                reserved,
            });
        }
    }

    for collection in &vpx.collections {
        if collection.name.chars().count() > MAX_NAME_LENGTH {
            findings.push(Kind::NameTooLong {
                item: format!("Collection {:?}", collection.name),
                length: collection.name.chars().count(),
            });
        }
    }
}

/// Names shared by several entries of a list, per list
pub(super) fn check_duplicate_names(vpx: &VPX, findings: &mut Vec<Kind>) {
    check_duplicates(
        vpx.images.iter().map(|image| image.name.as_str()),
        NameKind::Image,
        findings,
    );
    check_duplicates(
        vpx.sounds.iter().map(|sound| sound.name.as_str()),
        NameKind::Sound,
        findings,
    );
    check_duplicates(
        vpx.gameitems.iter().map(|item| item.name()),
        NameKind::GameItem,
        findings,
    );
    check_duplicates(
        vpx.collections.iter().map(|c| c.name.as_str()),
        NameKind::Collection,
        findings,
    );
    // a 10.8 table carries both lists but vpinball replaces the old one
    // with the new
    match &vpx.gamedata.materials {
        Some(materials) => check_duplicates(
            materials.iter().map(|material| material.name.as_str()),
            NameKind::Material,
            findings,
        ),
        None => check_duplicates(
            vpx.gamedata
                .materials_old
                .iter()
                .map(|material| material.name.as_str()),
            NameKind::Material,
            findings,
        ),
    }
}

/// A table without a name in its table info
pub(super) fn check_table_name(vpx: &VPX, findings: &mut Vec<Kind>) {
    if vpx
        .info
        .table_name
        .as_ref()
        .is_none_or(|name| name.is_empty())
    {
        findings.push(Kind::MissingTableName);
    }
}

/// The words VBScript reserves, lower cased
const VBS_KEYWORDS: &[&str] = &[
    "and",
    "as",
    "byref",
    "byval",
    "call",
    "case",
    "class",
    "const",
    "default",
    "dim",
    "do",
    "each",
    "else",
    "elseif",
    "empty",
    "end",
    "eqv",
    "erase",
    "error",
    "event",
    "exit",
    "explicit",
    "false",
    "for",
    "function",
    "get",
    "goto",
    "if",
    "imp",
    "implements",
    "in",
    "inherits",
    "is",
    "let",
    "like",
    "loop",
    "lset",
    "me",
    "mod",
    "new",
    "next",
    "not",
    "nothing",
    "null",
    "on",
    "option",
    "optional",
    "or",
    "paramarray",
    "preserve",
    "private",
    "property",
    "public",
    "raiseevent",
    "redim",
    "rem",
    "resume",
    "rset",
    "select",
    "set",
    "shared",
    "single",
    "static",
    "step",
    "stop",
    "sub",
    "then",
    "to",
    "true",
    "type",
    "typeof",
    "until",
    "variant",
    "wend",
    "while",
    "with",
    "xor",
];

/// The functions, objects and constants the VBScript engine provides,
/// lower cased, from wine's vbscript `global.c`, which mirrors Windows
const VBS_BUILTINS: &[&str] = &[
    "abs",
    "array",
    "asc",
    "ascb",
    "ascw",
    "atn",
    "cbool",
    "cbyte",
    "ccur",
    "cdate",
    "cdbl",
    "chr",
    "chrb",
    "chrw",
    "cint",
    "clng",
    "cos",
    "createobject",
    "csng",
    "cstr",
    "date",
    "dateadd",
    "datediff",
    "datepart",
    "dateserial",
    "datevalue",
    "day",
    "err",
    "escape",
    "eval",
    "execute",
    "executeglobal",
    "exp",
    "filter",
    "fix",
    "formatcurrency",
    "formatdatetime",
    "formatnumber",
    "formatpercent",
    "getlocale",
    "getobject",
    "getref",
    "hex",
    "hour",
    "inputbox",
    "instr",
    "instrb",
    "instrrev",
    "int",
    "isarray",
    "isdate",
    "isempty",
    "isnull",
    "isnumeric",
    "isobject",
    "join",
    "lbound",
    "lcase",
    "left",
    "leftb",
    "len",
    "lenb",
    "loadpicture",
    "log",
    "ltrim",
    "mid",
    "midb",
    "minute",
    "month",
    "monthname",
    "msgbox",
    "now",
    "oct",
    "randomize",
    "replace",
    "rgb",
    "right",
    "rightb",
    "rnd",
    "round",
    "rtrim",
    "scriptengine",
    "scriptenginebuildversion",
    "scriptenginemajorversion",
    "scriptengineminorversion",
    "second",
    "setlocale",
    "sgn",
    "sin",
    "space",
    "split",
    "sqr",
    "strcomp",
    "string",
    "strreverse",
    "tan",
    "time",
    "timer",
    "timeserial",
    "timevalue",
    "trim",
    "typename",
    "ubound",
    "ucase",
    "unescape",
    "vartype",
    "weekday",
    "weekdayname",
    "year",
];

/// The methods and properties of the table's global script object, plus
/// the `Debug` object, lower cased, from vpinball's `ITableGlobal`
/// interface
const TABLE_GLOBALS: &[&str] = &[
    "debug",
    // methods, which vpinball reserves itself on Windows
    "addobject",
    "beginmodal",
    "closeserial",
    "createpluginobject",
    "endmodal",
    "endmusic",
    "fireknocker",
    "flushserial",
    "getballs",
    "getcustomparam",
    "getelementbyname",
    "getelements",
    "getmaterial",
    "getmaterialphysics",
    "getserialdevices",
    "gettextfile",
    "loadtexture",
    "loadvalue",
    "materialcolor",
    "nudge",
    "nudgegetcalibration",
    "nudgesensorstatus",
    "nudgesetcalibration",
    "nudgetiltstatus",
    "openserial",
    "playmusic",
    "playsound",
    "quitplayer",
    "readserial",
    "savevalue",
    "setupserial",
    "stopsound",
    "updatematerial",
    "updatematerialphysics",
    "writeserial",
    // properties
    "activeball",
    "activetable",
    "addcreditkey",
    "addcreditkey2",
    "centertiltkey",
    "disablestaticprerendering",
    "dmdcoloredpixels",
    "dmdheight",
    "dmdpixels",
    "dmdwidth",
    "exitgame",
    "frameindex",
    "gametime",
    "getplayerhwnd",
    "joycustomkey",
    "leftflipperkey",
    "leftmagnasave",
    "lefttiltkey",
    "lockbarkey",
    "mechanicaltilt",
    "musicdirectory",
    "musicvolume",
    "nightday",
    "platformbits",
    "platformcpu",
    "platformos",
    "plungerkey",
    "precisegametime",
    "renderingmode",
    "rightflipperkey",
    "rightmagnasave",
    "righttiltkey",
    "scriptsdirectory",
    "setting",
    "showcursor",
    "showdt",
    "showfss",
    "stagedleftflipperkey",
    "stagedrightflipperkey",
    "startgamekey",
    "systemtime",
    "tablesdirectory",
    "userdirectory",
    "version",
    "versionmajor",
    "versionminor",
    "versionrevision",
    "vpbuildversion",
    "vpxactionkey",
    "windowheight",
    "windowwidth",
];

/// What the script already means by this name, if anything
fn reserved_name(name: &str) -> Option<ReservedName> {
    let lower = name.to_lowercase();
    let lower = lower.as_str();
    if VBS_KEYWORDS.contains(&lower) {
        Some(ReservedName::VbsKeyword)
    } else if VBS_BUILTINS.contains(&lower) {
        Some(ReservedName::VbsBuiltin)
    } else if TABLE_GLOBALS.contains(&lower) {
        Some(ReservedName::TableGlobal)
    } else {
        None
    }
}

pub(super) fn name_set<'a>(names: impl Iterator<Item = &'a str>) -> HashSet<String> {
    names.map(|name| name.to_lowercase()).collect()
}

pub(super) fn material_names(vpx: &VPX) -> HashSet<String> {
    let mut names: HashSet<String> = vpx
        .gamedata
        .materials_old
        .iter()
        .map(|material| material.name.to_lowercase())
        .collect();
    if let Some(materials) = &vpx.gamedata.materials {
        names.extend(
            materials
                .iter()
                .map(|material| material.name.to_lowercase()),
        );
    }
    names
}

fn check_duplicates<'a>(
    names: impl Iterator<Item = &'a str>,
    kind: NameKind,
    findings: &mut Vec<Kind>,
) {
    // the first spelling and how often the name occurs, in first seen order
    let mut seen: Vec<(&str, usize)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for name in names {
        if name.is_empty() {
            continue;
        }
        match index.get(&name.to_lowercase()) {
            Some(&at) => seen[at].1 += 1,
            None => {
                index.insert(name.to_lowercase(), seen.len());
                seen.push((name, 1));
            }
        }
    }
    for (name, count) in seen {
        if count > 1 {
            findings.push(Kind::DuplicateName {
                kind,
                name: name.to_string(),
                count,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::test_support::*;
    use crate::vpx::audit::{Severity, audit_kinds};
    use crate::vpx::gameitem::GameItemEnum;
    use pretty_assertions::assert_eq;

    #[test]
    fn unnamed_items_are_reported_per_type() {
        use crate::vpx::gameitem::decal::Decal;
        use crate::vpx::gameitem::wall::Wall;
        let mut vpx = clean_vpx();
        for _ in 0..2 {
            vpx.gameitems.push(GameItemEnum::Decal(Decal::default()));
        }
        vpx.gameitems.push(GameItemEnum::Wall(Wall::default()));
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![
                Kind::UnnamedItems {
                    type_name: "Decal".to_string(),
                    count: 2,
                },
                Kind::UnnamedItems {
                    type_name: "Wall".to_string(),
                    count: 1,
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
        assert_eq!(
            findings[0].to_string(),
            "2 Decal items have no name, saving the table in vpinball or vpx-editor names them"
        );
        assert_eq!(findings[1].severity(), Severity::Warning);
        assert_eq!(
            findings[1].to_string(),
            "1 Wall item has no name, they cannot be reached from the script"
        );
    }

    #[test]
    fn names_longer_than_the_editor_allows_are_reported() {
        use crate::vpx::collection::Collection;
        use crate::vpx::gameitem::wall::Wall;
        let mut vpx = clean_vpx();
        let wall = |name: String| {
            GameItemEnum::Wall(Wall {
                name,
                ..Default::default()
            })
        };
        vpx.gameitems.push(wall("w".repeat(31)));
        vpx.gameitems.push(wall("w".repeat(32)));
        vpx.collections.push(Collection {
            name: "c".repeat(32),
            items: Vec::new(),
            fire_events: false,
            stop_single_events: false,
            group_elements: false,
        });
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![
                Kind::NameTooLong {
                    item: format!("Wall {:?}", "w".repeat(32)),
                    length: 32,
                },
                Kind::NameTooLong {
                    item: format!("Collection {:?}", "c".repeat(32)),
                    length: 32,
                },
            ]
        );
        assert_eq!(
            findings[0].to_string(),
            format!(
                "Wall {:?}: name is 32 characters, vpinball cuts names at 31",
                "w".repeat(32)
            )
        );
    }

    #[test]
    fn names_the_script_already_means_something_by_are_reported() {
        use crate::vpx::collection::Collection;
        use crate::vpx::gameitem::timer::Timer;
        use crate::vpx::gameitem::wall::Wall;
        let mut vpx = clean_vpx();
        vpx.gameitems.push(GameItemEnum::Timer(Timer {
            name: "Timer".to_string(),
            ..Default::default()
        }));
        vpx.gameitems.push(GameItemEnum::Wall(Wall {
            name: "TO".to_string(),
            ..Default::default()
        }));
        vpx.collections.push(Collection {
            name: "GetBalls".to_string(),
            items: Vec::new(),
            fire_events: false,
            stop_single_events: false,
            group_elements: false,
        });
        let findings: Vec<Kind> = audit_kinds(&vpx)
            .into_iter()
            .filter(|finding| matches!(finding, Kind::ReservedName { .. }))
            .collect();
        assert_eq!(
            findings,
            vec![
                Kind::ReservedName {
                    kind: NameKind::GameItem,
                    name: "Timer".to_string(),
                    reserved: ReservedName::VbsBuiltin,
                },
                Kind::ReservedName {
                    kind: NameKind::GameItem,
                    name: "TO".to_string(),
                    reserved: ReservedName::VbsKeyword,
                },
                Kind::ReservedName {
                    kind: NameKind::Collection,
                    name: "GetBalls".to_string(),
                    reserved: ReservedName::TableGlobal,
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Warning);
        assert_eq!(
            findings[0].to_string(),
            "game item \"Timer\" is a VBScript builtin, the item hides it from the script"
        );
        assert_eq!(findings[1].severity(), Severity::Error);
        assert_eq!(
            findings[1].to_string(),
            "game item \"TO\" is a VBScript keyword, the script cannot refer to the item"
        );
        assert_eq!(findings[2].severity(), Severity::Error);
        assert_eq!(
            findings[2].to_string(),
            "collection \"GetBalls\" is a table script global, vpinball renames the item at load"
        );
    }

    #[test]
    fn duplicate_names_are_reported_case_insensitively() {
        let mut vpx = clean_vpx();
        for name in ["ding", "DING", "Ding"] {
            vpx.images.push(crate::vpx::image::ImageData {
                name: name.to_string(),
                ..Default::default()
            });
        }
        // the duplicates are referenced so only the duplicate is reported
        vpx.gamedata.image = "ding".to_string();
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::DuplicateName {
                kind: NameKind::Image,
                name: "ding".to_string(),
                count: 3,
            }]
        );
        assert_eq!(
            findings[0].to_string(),
            "3 images share the name \"ding\", vpinball keeps only the first one"
        );
    }

    #[test]
    fn duplicate_materials_are_reported_from_the_list_vpinball_uses() {
        use crate::vpx::material::{Material, SaveMaterial};
        let mut vpx = clean_vpx();
        // the old list of a 10.8 table repeats the new one, so counting
        // both would report every material
        let names = ["Apron", "apron", "Plastic"];
        vpx.gamedata.materials_old = names
            .iter()
            .map(|name| SaveMaterial {
                name: name.to_string(),
                ..Default::default()
            })
            .collect();
        vpx.gamedata.materials = Some(
            names
                .iter()
                .map(|name| {
                    let mut material = Material::default();
                    material.name = name.to_string();
                    material
                })
                .collect(),
        );
        let findings: Vec<Kind> = audit_kinds(&vpx)
            .into_iter()
            .filter(|finding| matches!(finding, Kind::DuplicateName { .. }))
            .collect();
        assert_eq!(
            findings,
            vec![Kind::DuplicateName {
                kind: NameKind::Material,
                name: "Apron".to_string(),
                count: 2,
            }]
        );
        assert_eq!(
            findings[0].to_string(),
            "2 materials share the name \"Apron\", vpinball keeps only the last one, older builds render it differently in the editor and the player"
        );

        // a table from before 10.8 only has the old list
        vpx.gamedata.materials = None;
        let findings: Vec<Kind> = audit_kinds(&vpx)
            .into_iter()
            .filter(|finding| matches!(finding, Kind::DuplicateName { .. }))
            .collect();
        assert_eq!(findings.len(), 1);
    }
}
