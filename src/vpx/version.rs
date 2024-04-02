use std::num::ParseIntError;
use std::path::PathBuf;
use std::{
    cmp,
    fmt::Display,
    io::{self, Read, Seek, Write},
    path::{Path, MAIN_SEPARATOR_STR},
};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use cfb::{CompoundFile, Stream};

#[derive(Debug, Clone, PartialEq)]
pub struct Version(u32);

impl Version {
    pub fn parse(version: &str) -> Result<Version, ParseIntError> {
        // TODO can we make more precise assumptions about the format?
        let version = version.parse::<u32>()?;
        Ok(Version(version))
    }

    pub fn to_u32_string(&self) -> String {
        self.0.to_string()
    }
}

impl Version {
    pub fn new(version: u32) -> Self {
        Version(version)
    }

    pub fn u32(&self) -> u32 {
        self.0
    }

    fn version_float(&self) -> f32 {
        (self.0 as f32) / 100f32
    }
}
impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version_float = self.version_float();
        write!(f, "{}", version_float)
    }
}
impl From<Version> for u32 {
    fn from(val: Version) -> Self {
        val.0
    }
}
impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

fn vpx_version_path() -> PathBuf {
    Path::new(MAIN_SEPARATOR_STR)
        .join("GameStg")
        .join("Version")
}

pub(crate) fn read_version<F: Read + Seek>(comp: &mut CompoundFile<F>) -> io::Result<Version> {
    let version_path = vpx_version_path();
    let mut stream = comp.open_stream(version_path)?;
    read_version_data(&mut stream)
}

pub(crate) fn write_version<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    version: &Version,
) -> io::Result<()> {
    // we expect GameStg to exist
    let version_path = vpx_version_path();
    let mut stream = comp.create_stream(version_path)?;
    write_version_data(version, &mut stream)
}

fn read_version_data<F: Read + Seek>(stream: &mut Stream<F>) -> io::Result<Version> {
    let version = stream.read_u32::<LittleEndian>()?;
    Ok(Version(version))
}

fn write_version_data<F: Read + Write + Seek>(
    version: &Version,
    stream: &mut Stream<F>,
) -> io::Result<()> {
    stream.write_u32::<LittleEndian>(version.0)?;
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use testresult::TestResult;

    #[test]
    pub fn test_parse_invalid() {
        let version_string = "invalid";
        let parsed_version = Version::parse(version_string);
        assert!(parsed_version.is_err());
        let message = parsed_version.unwrap_err().to_string();
        assert_eq!(message, "invalid digit found in string");
    }

    #[test]
    pub fn test_to_string_parse() -> TestResult {
        let version = Version::new(1080);
        let version_string = version.to_u32_string();
        let parsed_version = Version::parse(&version_string)?;
        assert_eq!(version, parsed_version);
        Ok(())
    }

    #[test]
    pub fn test_parse_to_string() -> TestResult {
        let version_string = "1080";
        let parsed_version = Version::parse(version_string)?;
        let version_string2 = parsed_version.to_u32_string();
        assert_eq!(version_string, version_string2);
        Ok(())
    }
}
