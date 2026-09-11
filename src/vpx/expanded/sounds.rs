//! Sound reading and writing for expanded VPX format

use crate::filesystem::FileSystem;
use crate::vpx::sound::{SoundData, SoundDataJson, read_sound, write_sound};
use log::info;
use std::borrow::Cow;
use std::collections::HashSet;
use std::io;
use std::path::Path;

use super::util::{read_json, sanitize_filename};
use super::{Output, SOUNDS_DIR, WriteError};

pub(super) fn write_sounds(sounds: &[SoundData], out: &Output) -> Result<(), WriteError> {
    let mut sound_names_lower: HashSet<String> = HashSet::new();
    let mut sound_names_dupe_counter = 0;
    let mut json_sounds = Vec::with_capacity(sounds.len());
    let sounds: Vec<(String, &SoundData)> = sounds
        .iter()
        .map(|sound| {
            let mut json = SoundDataJson::from_sound_data(sound);
            let name_sanitized = sanitize_filename(&sound.name);
            if name_sanitized != sound.name {
                info!(
                    "Sound name {} contained invalid characters, sanitized to {}",
                    sound.name, name_sanitized
                );
                json.name_dedup = Some(name_sanitized.clone());
            }
            let lower_name = name_sanitized.to_lowercase();
            if sound_names_lower.contains(&lower_name) {
                sound_names_dupe_counter += 1;
                let name_dedup = format!("{}_dedup{}", sound.name, sound_names_dupe_counter);
                info!(
                    "Sound name {} is not unique, renaming file to {}",
                    name_sanitized, name_dedup
                );
                json.name_dedup = Some(name_dedup);
            }
            sound_names_lower.insert(lower_name);

            let actual_name = json.name_dedup.as_ref().unwrap_or(&name_sanitized);
            let file_name = format!("{}.{}", actual_name, sound.ext());
            json_sounds.push(json);
            (file_name, sound)
        })
        .collect();
    out.write_json(Path::new("sounds.json"), || json_sounds)?;

    sounds.iter().try_for_each(|(sound_file_name, sound)| {
        let sound_path = Path::new(SOUNDS_DIR).join(sound_file_name);
        out.write_with(&sound_path, || {
            if out.exists(&sound_path) {
                return Err(WriteError::Io(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "Two sounds with the same name detected, should not happen: {}",
                        out.path(&sound_path).display()
                    ),
                )));
            }
            Ok(Cow::Owned(write_sound(sound)))
        })
    })?;
    Ok(())
}

pub(super) fn read_sounds<P: AsRef<Path>>(
    expanded_dir: &P,
    fs: &dyn FileSystem,
) -> io::Result<Vec<SoundData>> {
    let sounds_json_path = expanded_dir.as_ref().join("sounds.json");
    if !fs.exists(&sounds_json_path) {
        info!("No sounds.json found");
        return Ok(vec![]);
    }
    let sounds_json: Vec<SoundDataJson> = read_json(&sounds_json_path, fs)?;
    let sounds_dir = expanded_dir.as_ref().join("sounds");
    let sounds: io::Result<Vec<SoundData>> = sounds_json
        .into_iter()
        .map(|sound_data_json| {
            let mut sound = sound_data_json.to_sound_data();
            let file_name = sound_data_json.name_dedup.as_ref().unwrap_or(&sound.name);
            let full_file_name = format!("{}.{}", file_name, sound.ext());
            let file_path = sounds_dir.join(full_file_name);
            if fs.exists(&file_path) {
                let sound_data = fs.read_file(&file_path)?;
                read_sound(&sound_data, &mut sound)?;
                Ok(sound)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Sound file not found: {}", file_path.display()),
                ))
            }
        })
        .collect();
    sounds
}
