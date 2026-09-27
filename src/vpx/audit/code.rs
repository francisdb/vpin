//! Checks on the text of the script that need no parser: deprecated
//! properties and mixed line endings.

use super::{Kind, VPX};
use std::collections::HashSet;

/// A script that mixes line ending styles
pub(super) fn check_line_endings(vpx: &VPX, findings: &mut Vec<Kind>) {
    let script = &vpx.gamedata.code.string;
    let crlf = script.matches("\r\n").count();
    let lf = script.matches('\n').count() - crlf;
    let cr = script.matches('\r').count() - crlf;
    if [crlf, lf, cr].iter().filter(|count| **count > 0).count() > 1 {
        findings.push(Kind::MixedScriptLineEndings { crlf, lf, cr });
    }
}

/// Table properties vpinball logs "is deprecated" for and ignores
/// (pintable.cpp); the camera ones moved to the view setups
const DEPRECATED_TABLE_PROPERTIES: &[&str] = &[
    "3DOffset",
    "BackglassMode",
    "EnableAntialiasing",
    "EnableFXAA",
    "FieldOfView",
    "GlobalAlphaAcc",
    "GlobalDayNight",
    "GlobalStereo3D",
    "Inclination",
    "Layback",
    "MaxSeparation",
    "PlungerFilter",
    "PlungerNormalize",
    "ReflectElementsOnPlayfield",
    "Rotation",
    "Scalex",
    "Scaley",
    "Scalez",
    "TableAdaptiveVSync",
    "TableHeight",
    "Xlatex",
    "Xlatey",
    "Xlatez",
    "YieldTime",
    "ZPD",
];

/// VPinMAME controller properties the standalone PinMAME plugin logs
/// "is deprecated" for (plugins/pinmame/Controller.h); they concern the
/// VPinMAME window, which the plugin does not have
const DEPRECATED_CONTROLLER_PROPERTIES: &[&str] = &[
    "CabinetMode",
    "DoubleSize",
    "FastFrames",
    "HandleKeyboard",
    "IgnoreRomCrc",
    "LockDisplay",
    "ShowDMDOnly",
    "ShowFrame",
    "ShowOptsDialog",
    "ShowTitle",
    "SoundMode",
];

/// The code of a script with comments and string literals removed, lower
/// cased, one line per line
fn script_code(script: &str) -> String {
    let mut code = String::with_capacity(script.len());
    for line in script.lines() {
        let mut in_string = false;
        for c in line.chars() {
            match (in_string, c) {
                (false, '"') => in_string = true,
                (false, '\'') => break,
                (false, c) => code.push(c.to_ascii_lowercase()),
                (true, '"') => in_string = false,
                (true, _) => {}
            }
        }
        code.push('\n');
    }
    code
}

/// Deprecated properties accessed on the table object, by its name
/// (`Table1.Inclination`); other objects have properties with some of
/// these names, so only the table qualified form counts
pub(super) fn check_deprecated_properties(vpx: &VPX, findings: &mut Vec<Kind>) {
    let table = vpx.gamedata.name.to_lowercase();
    if table.is_empty() {
        return;
    }
    let code = script_code(&vpx.gamedata.code.string);
    let prefix = format!("{table}.");
    let mut reported: HashSet<&str> = HashSet::new();
    let mut rest = code.as_str();
    while let Some(position) = rest.find(&prefix) {
        let word_start = position == 0
            || !rest[..position]
                .chars()
                .last()
                .is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.');
        let after = &rest[position + prefix.len()..];
        let property: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if word_start
            && let Some(known) = DEPRECATED_TABLE_PROPERTIES
                .iter()
                .find(|known| known.eq_ignore_ascii_case(&property))
            && reported.insert(known)
        {
            findings.push(Kind::DeprecatedTableProperty {
                property: (*known).to_string(),
            });
        }
        rest = after;
    }
}

/// Controller properties are set on whatever variable holds the
/// controller, usually inside `With Controller`, so any `.name` member
/// access counts; the names are specific to VPinMAME
pub(super) fn check_deprecated_controller_properties(vpx: &VPX, findings: &mut Vec<Kind>) {
    let code = script_code(&vpx.gamedata.code.string);
    let mut reported: HashSet<&str> = HashSet::new();
    for word in code.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.') {
        let Some(member) = word.rsplit('.').next().filter(|_| word.contains('.')) else {
            continue;
        };
        if let Some(known) = DEPRECATED_CONTROLLER_PROPERTIES
            .iter()
            .find(|known| known.eq_ignore_ascii_case(member))
            && reported.insert(known)
        {
            findings.push(Kind::DeprecatedControllerProperty {
                property: (*known).to_string(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::audit::test_support::*;
    use crate::vpx::audit::{Severity, audit_kinds};
    use pretty_assertions::assert_eq;

    #[test]
    fn mixed_script_line_endings_are_reported() {
        let mut vpx = clean_vpx();
        // valid VBScript with Option Explicit so only the line-ending check
        // fires, with a bare LF and a bare CR mixed into the CRLF endings
        vpx.gamedata.code.string =
            "Option Explicit\r\nRandomize\nRandomize\rRandomize\r\n".to_string();
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![Kind::MixedScriptLineEndings {
                crlf: 2,
                lf: 1,
                cr: 1
            }]
        );
        assert_eq!(findings[0].severity(), Severity::Suggestion);
        assert_eq!(
            findings[0].to_string(),
            "script mixes line endings: 2 CRLF, 1 LF, 1 CR"
        );
    }

    #[test]
    fn consistent_lf_line_endings_are_fine() {
        let mut vpx = clean_vpx();
        vpx.gamedata.code.string = "Option Explicit\nRandomize\nRandomize\n".to_string();
        assert_eq!(audit_kinds(&vpx), vec![]);
    }

    #[test]
    fn deprecated_table_properties_are_reported_once() {
        let mut vpx = clean_vpx();
        vpx.gamedata.name = "Table1".to_string();
        vpx.gamedata.set_code(
            "Option Explicit\r\nTABLE1.Inclination = 42\r\nTable1.Inclination = 43\r\nTable1.Layback = 1 ' Table1.ZPD = 2\r\nx = \"Table1.YieldTime\"\r\nMyTable1.Rotation = 1\r\nPrimitive1.Rotation = 90\r\nTable1.Name = \"x\"\r\n"
                .to_string(),
        );
        assert_eq!(
            audit_kinds(&vpx),
            vec![
                Kind::DeprecatedTableProperty {
                    property: "Inclination".to_string(),
                },
                Kind::DeprecatedTableProperty {
                    property: "Layback".to_string(),
                },
            ]
        );
    }

    #[test]
    fn deprecated_controller_properties_are_informational() {
        let mut vpx = clean_vpx();
        vpx.gamedata.set_code(
            "Option Explicit\r\nWith Controller\r\n    .ShowTitle = False\r\n    .ShowDMDOnly = 1 : .ShowFrame = 0\r\n    .HandleKeyboard = 0\r\n    .Run\r\nEnd With\r\n' .ShowTitle in a comment\r\nx = \"ShowTitle\"\r\n"
                .to_string(),
        );
        let findings = audit_kinds(&vpx);
        assert_eq!(
            findings,
            vec![
                Kind::DeprecatedControllerProperty {
                    property: "ShowTitle".to_string(),
                },
                Kind::DeprecatedControllerProperty {
                    property: "ShowDMDOnly".to_string(),
                },
                Kind::DeprecatedControllerProperty {
                    property: "ShowFrame".to_string(),
                },
                Kind::DeprecatedControllerProperty {
                    property: "HandleKeyboard".to_string(),
                },
            ]
        );
        assert_eq!(findings[0].severity(), Severity::Info);
    }
}
