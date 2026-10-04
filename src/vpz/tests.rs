use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use testresult::TestResult;

fn properties(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(properties) => properties,
        other => panic!("expected an object, got {other}"),
    }
}

fn text(files: &BTreeMap<String, Vec<u8>>, path: &str) -> String {
    let data = files
        .get(path)
        .unwrap_or_else(|| panic!("{path} should be written, got {:?}", files.keys()));
    String::from_utf8(data.clone()).expect("UTF-8")
}

fn json_file(files: &BTreeMap<String, Vec<u8>>, path: &str) -> Value {
    serde_json::from_str(&text(files, path)).expect("JSON")
}

fn files(entries: &[(&str, &[u8])]) -> BTreeMap<String, Vec<u8>> {
    entries
        .iter()
        .map(|(path, data)| (path.to_string(), data.to_vec()))
        .collect()
}

const MANIFEST_JSON: &[u8] =
    br#"{"$type": "manifest", "file_format": "vpinball-pack", "file_version": 1}"#;

fn part(name: &str, part_type: PartType) -> Part {
    Part {
        name: name.to_string(),
        part_type,
        properties: properties(json!({"center": {"x": 1.5, "y": 2.0}, "visible": true})),
        mesh: None,
    }
}

fn named(name: &str) -> NamedDocument {
    NamedDocument {
        name: name.to_string(),
        properties: properties(json!({"type": 0, "roughness": 0.5})),
    }
}

fn sample() -> Vpz {
    Vpz {
        manifest: Manifest {
            name: Some("Sample".to_string()),
            save_date: Some("Sun Oct  4 12:00:00 2026".to_string()),
            extra: properties(json!({"future": [1, 2]})),
            ..Manifest::default()
        },
        table: Some(Document {
            properties: properties(json!({
                "left": 0.0,
                "parts": [],
                "desktop_view": {"mode": 0, "fov": 45.0},
                "vbs_script": "script.vbs",
                "table_name": "Sample",
                "custom_tags": {"tag": "value"},
            })),
        }),
        script: Some("Option Explicit\r\n' Mélo\r\n".to_string()),
        parts: vec![
            part("Wall", PartType::Surface),
            part("wall", PartType::Surface),
            part("a/b", PartType::Light),
            part("a_b", PartType::Light),
            part("Mélo", PartType::Flipper),
            Part {
                mesh: Some(b"glTF fake".to_vec()),
                ..part("Prim", PartType::Primitive)
            },
            part("Future", PartType::Other("hologram".to_string())),
        ],
        collections: vec![NamedDocument {
            name: "GI".to_string(),
            properties: properties(json!({"parts": ["Wall", "a/b"], "fire_events": false})),
        }],
        materials: vec![named("Metal"), named("metal ")],
        render_probes: vec![named("Playfield Reflections")],
        images: vec![
            Asset {
                name: "noext".to_string(),
                extension: String::new(),
                data: vec![1, 2, 3],
                sidecar: ImageSidecar::default(),
            },
            Asset {
                name: "playfield".to_string(),
                extension: "webp".to_string(),
                data: b"RIFF....WEBP".to_vec(),
                sidecar: ImageSidecar {
                    import_path: Some("C:\\tables\\playfield.webp".to_string()),
                    width: Some(1024),
                    height: Some(2048),
                    alpha_test: Some(-255.0),
                    md5: Some("00112233445566778899aabbccddeeff".to_string()),
                    opaque: Some(true),
                    link: None,
                    extra: Map::new(),
                },
            },
        ],
        sounds: vec![Asset {
            name: "fx_bumper".to_string(),
            extension: "wav".to_string(),
            data: b"RIFF....WAVE".to_vec(),
            sidecar: SoundSidecar {
                output_target: Some(OutputTarget::Backglass),
                volume_offset: Some(-3),
                left_right_offset: Some(100),
                rear_front_offset: Some(-100),
                ..SoundSidecar::default()
            },
        }],
        fonts: vec![Asset {
            name: "digits".to_string(),
            extension: "ttf".to_string(),
            data: vec![0, 1, 0, 0],
            sidecar: FontSidecar::default(),
        }],
        other_files: files(&[("README.txt", b"hello"), ("images/notes.json", b"{}")]),
    }
}

#[test]
fn a_pack_round_trips_through_its_files() -> TestResult {
    let vpz = sample();
    let files = to_files(&vpz)?;
    let mut expected = vpz;
    // the writer derives the name lists of the table
    if let Some(table) = expected.table.as_mut() {
        for (key, names) in [
            (
                "parts",
                json!(["Wall", "wall", "a/b", "a_b", "Mélo", "Prim", "Future"]),
            ),
            ("collections", json!(["GI"])),
            ("materials", json!(["Metal", "metal "])),
            ("renderprobes", json!(["Playfield Reflections"])),
        ] {
            table.properties.insert(key.to_string(), names);
        }
    }
    assert_eq!(from_files(files)?, expected);
    Ok(())
}

#[test]
fn files_follow_the_vpinball_layout() -> TestResult {
    let files = to_files(&sample())?;
    let paths: Vec<&str> = files.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        vec![
            "README.txt",
            "collections/GI.json",
            "fonts/digits.json",
            "fonts/digits.ttf",
            "images/noext",
            "images/noext.json",
            "images/notes.json",
            "images/playfield.json",
            "images/playfield.webp",
            "manifest.json",
            "materials/Metal.json",
            "materials/metal_2.json",
            "meshes/Prim.glb",
            "parts/Future.json",
            "parts/M__lo.json",
            "parts/Prim.json",
            "parts/Wall.json",
            "parts/a_b.json",
            "parts/a_b_2.json",
            "parts/wall_2.json",
            "renderprobes/Playfield Reflections.json",
            "script.vbs",
            "sounds/fx_bumper.json",
            "sounds/fx_bumper.wav",
            "table.json",
        ]
    );
    assert_eq!(
        text(&files, "parts/Wall.json"),
        "{\n  \"$type\": \"surface\",\n  \"center\": {\n    \"x\": 1.5,\n    \"y\": 2.0\n  },\n  \"visible\": true\n}"
    );
    // a listed name the sanitized file name resolves to is not written, as
    // in vpinball, a collision suffixed one is
    assert_eq!(json_file(&files, "parts/a_b.json").get("name"), None);
    assert_eq!(json_file(&files, "parts/M__lo.json").get("name"), None);
    assert_eq!(json_file(&files, "parts/wall_2.json")["name"], "wall");
    assert_eq!(json_file(&files, "parts/a_b_2.json")["name"], "a_b");
    assert_eq!(
        json_file(&files, "materials/metal_2.json")["name"],
        "metal "
    );
    assert_eq!(
        json_file(&files, "parts/Prim.json")["mesh"],
        "meshes/Prim.glb"
    );
    assert_eq!(
        json_file(&files, "manifest.json"),
        json!({
            "$type": "manifest",
            "file_format": "vpinball-pack",
            "file_version": 1,
            "name": "Sample",
            "save_date": "Sun Oct  4 12:00:00 2026",
            "future": [1, 2],
        })
    );
    let sound = text(&files, "sounds/fx_bumper.json");
    assert_eq!(
        sound,
        "{\n  \"$type\": \"sound\",\n  \"output_target\": \"backglass\",\n  \"volume_offset\": -3,\n  \"left_right_offset\": 100,\n  \"rear_front_offset\": -100\n}"
    );
    assert_eq!(text(&files, "script.vbs"), "Option Explicit\r\n' Mélo\r\n");
    Ok(())
}

#[test]
fn table_keys_keep_their_position() -> TestResult {
    let files = to_files(&sample())?;
    let table = text(&files, "table.json");
    let keys: Vec<String> = match serde_json::from_str::<Value>(&table)? {
        Value::Object(properties) => properties.keys().cloned().collect(),
        _ => panic!("table.json should be an object"),
    };
    assert_eq!(
        keys,
        vec![
            "$type",
            "left",
            "parts",
            "desktop_view",
            "vbs_script",
            "table_name",
            "custom_tags",
            "collections",
            "materials",
            "renderprobes",
        ]
    );
    Ok(())
}

/// A pack as vpinball writes it, plus what an author may add by hand
#[test]
fn a_vpinball_pack_is_read() -> TestResult {
    let vpz = from_files(files(&[
        ("manifest.json", MANIFEST_JSON),
        (
            "table.json",
            br#"{"$type": "table", "vbs_script": "Sub Inline\r\nEnd Sub",
                 "parts": ["Zeta", "a/b", "Missing", "Prim"],
                 "materials": ["Steel"]}"#,
        ),
        ("parts/Alpha.json", br#"{"$type": "kicker"}"#),
        ("parts/Zeta.json", br#"{"$type": "gate", "z": 1, "a": 2}"#),
        ("parts/a_b.json", br#"{"$type": "light"}"#),
        (
            "parts/Prim.json",
            br#"{"$type": "primitive", "use_mesh": true}"#,
        ),
        ("parts/nested/Deep.json", br#"{"$type": "timer"}"#),
        ("meshes/Prim.glb", b"glb"),
        (
            "materials/Steel.json",
            br#"{"$type": "material", "name": "Steel Plate"}"#,
        ),
        ("materials/Steel 2.json", br#"{"$type": "material"}"#),
        ("images/plain.png", b"png"),
        ("images/lonely.json", br#"{"$type": "image"}"#),
        ("images/both.jpg", b"jpg"),
        (
            "images/both.json",
            br#"{"$type": "image", "width": 4, "flag": "kept"}"#,
        ),
    ]))?;

    assert_eq!(vpz.script.as_deref(), Some("Sub Inline\r\nEnd Sub"));
    let part_names: Vec<(&str, &PartType)> = vpz
        .parts
        .iter()
        .map(|part| (part.name.as_str(), &part.part_type))
        .collect();
    assert_eq!(
        part_names,
        vec![
            ("Zeta", &PartType::Gate),
            ("a/b", &PartType::Light),
            ("Prim", &PartType::Primitive),
            ("Alpha", &PartType::Kicker),
            ("Deep", &PartType::Timer),
        ]
    );
    let zeta_keys: Vec<&String> = vpz.parts[0].properties.keys().collect();
    assert_eq!(zeta_keys, vec!["z", "a"]);
    assert_eq!(vpz.parts[2].mesh.as_deref(), Some(b"glb".as_slice()));

    // a document name wins over the list and the file name
    let materials: Vec<&str> = vpz.materials.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(materials, vec!["Steel Plate", "Steel 2"]);

    let images: Vec<(&str, &str)> = vpz
        .images
        .iter()
        .map(|image| (image.name.as_str(), image.extension.as_str()))
        .collect();
    assert_eq!(images, vec![("both", "jpg"), ("plain", "png")]);
    assert_eq!(vpz.images[0].sidecar.width, Some(4));
    assert_eq!(
        vpz.images[0].sidecar.extra,
        properties(json!({"flag": "kept"}))
    );
    assert_eq!(vpz.images[1].sidecar, ImageSidecar::default());
    let others: Vec<&String> = vpz.other_files.keys().collect();
    assert_eq!(others, vec!["images/lonely.json"]);
    Ok(())
}

#[test]
fn a_partial_pack_has_no_table() -> TestResult {
    let vpz = from_files(files(&[
        ("manifest.json", MANIFEST_JSON),
        ("parts/Wall.json", br#"{"$type": "surface"}"#),
        ("script.vbs", b"' script"),
    ]))?;
    assert_eq!(vpz.table, None);
    assert_eq!(vpz.parts.len(), 1);
    assert_eq!(vpz.script.as_deref(), Some("' script"));
    assert_eq!(from_files(to_files(&vpz)?)?, vpz);
    Ok(())
}

fn read_error(entries: &[(&str, &[u8])]) -> String {
    match from_files(files(entries)) {
        Ok(_) => panic!("the pack should be rejected"),
        Err(error) => {
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            error.to_string()
        }
    }
}

#[test]
fn broken_packs_are_rejected() {
    assert!(read_error(&[("table.json", b"{}")]).contains("manifest.json is missing"));
    assert!(
        read_error(&[(
            "manifest.json",
            br#"{"file_format": "zip", "file_version": 1}"#
        )])
        .contains("file_format \"zip\"")
    );
    assert!(
        read_error(&[(
            "manifest.json",
            br#"{"file_format": "vpinball-pack", "file_version": 2}"#
        )])
        .contains("file_version 2 is newer")
    );
    assert!(
        read_error(&[("manifest.json", MANIFEST_JSON), ("parts/Wall.json", b"{")])
            .contains("parts/Wall.json")
    );
    assert!(
        read_error(&[("manifest.json", MANIFEST_JSON), ("parts/Wall.json", b"{}")])
            .contains("the part has no $type")
    );
    assert!(
        read_error(&[
            ("manifest.json", MANIFEST_JSON),
            (
                "parts/Prim.json",
                br#"{"$type": "primitive", "mesh": "meshes/Gone.glb"}"#
            ),
        ])
        .contains("meshes/Gone.glb is missing")
    );
}

#[test]
fn other_files_cannot_replace_pack_files() {
    let mut vpz = sample();
    vpz.other_files
        .insert("script.vbs".to_string(), b"' sneaky".to_vec());
    let error = to_files(&vpz).expect_err("a duplicate path should be an error");
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
}

#[test]
fn a_pack_round_trips_through_a_zip() -> TestResult {
    let vpz = sample();
    let bytes = to_zip_bytes(&vpz)?;
    assert_eq!(from_zip_bytes(&bytes)?, from_files(to_files(&vpz)?)?);
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes))?;
    assert_eq!(archive.by_index(0)?.name(), "manifest.json");
    // no timestamps, so the same pack gives the same bytes
    assert_eq!(to_zip_bytes(&vpz)?, bytes);
    Ok(())
}

#[test]
fn zip_entries_outside_the_pack_are_rejected() -> TestResult {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())?;
    zip.write_all(MANIFEST_JSON)?;
    zip.start_file("../evil.json", zip::write::SimpleFileOptions::default())?;
    zip.write_all(b"{}")?;
    let bytes = zip.finish()?.into_inner();
    let error = from_zip_bytes(&bytes).expect_err("an escaping entry should be an error");
    assert!(error.to_string().contains("../evil.json"), "{error}");
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
#[test]
fn a_pack_round_trips_through_a_directory_and_a_file() -> TestResult {
    let root = testdir::testdir!();
    let vpz = from_files(to_files(&sample())?)?;

    let dir = root.join("folder");
    write(&vpz, &dir)?;
    assert!(dir.join("parts").join("M__lo.json").is_file());
    assert_eq!(read(&dir)?, vpz);

    let error = write(&vpz, &dir).expect_err("a non-empty directory should be refused");
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);

    let file = root.join("table.vpz");
    write(&vpz, &file)?;
    assert!(file.is_file());
    assert_eq!(read(&file)?, vpz);
    Ok(())
}

#[test]
fn a_sanitized_asset_name_is_written_in_its_sidecar() -> TestResult {
    let vpz = Vpz {
        images: vec![Asset {
            name: "light?".to_string(),
            extension: "png".to_string(),
            data: vec![1],
            sidecar: ImageSidecar::default(),
        }],
        ..Vpz::default()
    };
    let files = to_files(&vpz)?;
    // no name list covers assets, so the file stem alone would lose it
    assert_eq!(json_file(&files, "images/light_.json")["name"], "light?");
    assert_eq!(from_files(files)?.images[0].name, "light?");
    Ok(())
}

#[test]
fn a_sanitized_name_without_a_table_is_written_in_its_document() -> TestResult {
    let vpz = Vpz {
        parts: vec![part("a/b", PartType::Light)],
        ..Vpz::default()
    };
    let files = to_files(&vpz)?;
    assert_eq!(json_file(&files, "parts/a_b.json")["name"], "a/b");
    assert_eq!(from_files(files)?.parts[0].name, "a/b");
    Ok(())
}

/// `testdata/completely_blank_table_10_7_4.vpx` as vpinball master saves
/// it as a .vpz, with vpx-test
const VPINBALL_PACK: &[u8] = include_bytes!("../../testdata/vpz/completely_blank_table_10_7_4.vpz");

#[test]
fn a_vpinball_pack_is_written_back_byte_for_byte() -> TestResult {
    let mut archive = zip::ZipArchive::new(Cursor::new(VPINBALL_PACK))?;
    let mut original = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        original.insert(entry.name().to_string(), data);
    }
    let written = to_files(&from_zip_bytes(VPINBALL_PACK)?)?;
    assert_eq!(
        written.keys().collect::<Vec<_>>(),
        original.keys().collect::<Vec<_>>()
    );
    for (path, data) in &original {
        assert!(
            written[path] == *data,
            "{path} differs:\n{}",
            String::from_utf8_lossy(&written[path])
        );
    }
    Ok(())
}
