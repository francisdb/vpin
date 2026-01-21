//! Library for reading and writing [Visual Pinball](https://github.com/vpinball/vpinball/) `vpx` files and working with exploded vpx directories.
//!
//! # Example
//!
//! ```no_run
//! use std::io;
//! use std::path::PathBuf;
//! use vpin::vpx::{read, version};
//!
//! let path = PathBuf::from("testdata/completely_blank_table_10_7_4.vpx");
//! let vpx = read(&path).unwrap();
//! println!("version: {}", vpx.version);
//! println!("table name: {}", vpx.info.table_name.unwrap_or("unknown".to_string()));
//! ```
//!

use ::image::ImageFormat;
use std::fs::OpenOptions;
use std::io::{self, Error, Read, Seek, Write};
use std::path::MAIN_SEPARATOR_STR;
use std::{
    fs::File,
    path::{Path, PathBuf},
};

use cfb::{CompoundFile, CreateStreamOptions, OpenStreamOptions};
use log::{debug, info, warn};
use md2::{Digest, Md2};

use crate::vpx::biff::BiffReader;

use crate::vpx::expanded::vpx_image_to_dynamic_image;
use crate::vpx::image::ImageDataJpeg;
use crate::vpx::tableinfo::read_tableinfo;
use tableinfo::{TableInfo, write_tableinfo};
use version::Version;

use self::biff::{BiffRead, BiffWrite, BiffWriter};
use self::collection::Collection;
use self::custominfotags::CustomInfoTags;
use self::font::FontData;
use self::gamedata::GameData;
use self::gameitem::GameItemEnum;
use self::image::ImageData;
use self::sound::SoundData;
use self::version::{read_version, write_version};

pub mod biff;
pub mod collection;
pub mod color;
pub mod custominfotags;
pub mod expanded;
pub mod font;
pub mod gamedata;
pub mod gameitem;
pub mod image;
pub mod jsonmodel;
pub mod math;
pub mod model;
pub mod sound;
pub mod tableinfo;
pub mod version;

pub mod material;

pub mod renderprobe;

pub(crate) mod json;

// we have to make this public for the integration tests
pub mod lzw;
mod obj;
pub(crate) mod wav;

/// Convert from visual pinball units to millimeters
#[inline(always)]
pub fn mm_to_vpu(x: f32) -> f32 {
    x * (50.0 / (25.4 * 1.0625))
}

/// Convert from visual pinball units to millimeters
#[inline(always)]
pub fn vpu_to_mm(x: f32) -> f32 {
    x * (25.4 * 1.0625 / 50.0)
}

/// Convert from meters to visual pinball units
#[inline(always)]
pub fn m_to_vpu(x: f32) -> f32 {
    x * 50.0 / (0.0254 * 1.0625)
}

/// Convert from visual pinball units to meters
#[inline(always)]
pub fn vpu_to_m(x: f32) -> f32 {
    x * 0.0254 * 1.0625 / 50.0
}

/// In-memory representation of a VPX file
///
/// *We guarantee an exact copy when reading and writing this. Exact as in the same structure and data, the underlying compound file will be a bit different on the binary level.*
///
/// # Example
///
/// ```no_run
/// use std::io;
/// use std::path::PathBuf;
/// use vpin::vpx::{read, version};
///
/// let path = PathBuf::from("testdata/completely_blank_table_10_7_4.vpx");
/// let vpx = read(&path).unwrap();
/// println!("version: {}", vpx.version);
/// println!("table name: {}", vpx.info.table_name.unwrap_or("unknown".to_string()));
/// ```

#[derive(Debug, PartialEq, Default)]
pub struct VPX {
    /// This is mainly here to have an ordering for custom info tags
    pub custominfotags: CustomInfoTags, // this is a bit redundant
    pub info: TableInfo,
    pub version: Version,
    pub gamedata: GameData,
    pub gameitems: Vec<GameItemEnum>,
    pub images: Vec<ImageData>,
    pub sounds: Vec<SoundData>,
    pub fonts: Vec<FontData>,
    pub collections: Vec<Collection>,
}

pub enum AddImageResult {
    Added,
    Replaced(Box<ImageData>),
}

impl VPX {
    pub fn add_game_item(&mut self, item: GameItemEnum) -> &Self {
        self.gameitems.push(item);
        self.gamedata.gameitems_size = self.gameitems.len() as u32;
        self
    }

    pub fn set_script(&mut self, script: String) -> &Self {
        self.gamedata.set_code(script);
        self
    }

    pub fn add_or_replace_image(&mut self, image: ImageData) -> AddImageResult {
        // make sure there is a unique name
        let existing_pos = self
            .images
            .iter()
            .position(|i| i.name.eq_ignore_ascii_case(&image.name));
        match existing_pos {
            Some(pos) => {
                let existing = self.images[pos].clone();
                self.images[pos] = image;
                AddImageResult::Replaced(Box::new(existing))
            }
            None => {
                self.gamedata.images_size += 1;
                self.images.push(image);
                AddImageResult::Added
            }
        }
    }
}

#[derive(Debug)]
pub enum ExtractResult {
    Extracted(PathBuf),
    Existed(PathBuf),
}

#[derive(Eq, PartialEq, Debug)]
pub enum VerifyResult {
    Ok(PathBuf),
    Failed(PathBuf, String),
}

/// Handle to an underlying VPX file
///
/// # Example
///
/// ```no_run
/// use std::io;
/// use std::path::PathBuf;
/// use vpin::vpx::{open, read, version};
///
/// let path = PathBuf::from("testdata/completely_blank_table_10_7_4.vpx");
/// let mut vpx = open(&path).unwrap();
/// let version = vpx.read_version().unwrap();
/// println!("version: {}", version);
/// let images = vpx.read_images().unwrap();
/// for image in images {
///    println!("image: {}", image.name);
/// }
/// ```
///
pub struct VpxFile<F> {
    // keep this private
    compound_file: CompoundFile<F>,
}

impl<F: Read + Seek + Write> VpxFile<F> {
    /// Opens an existing compound file, using the underlying reader.  If the
    /// underlying reader also supports the `Write` trait, then the
    /// `CompoundFile` object will be writable as well.
    pub fn open(inner: F) -> io::Result<VpxFile<F>> {
        // TODO the fact that this is read only should be reflected in the VpxFile type
        let compound_file = CompoundFile::open_strict(inner)?;
        Ok(VpxFile { compound_file })
    }

    pub fn open_rw(inner: F) -> io::Result<VpxFile<F>> {
        let compound_file = CompoundFile::open_strict(inner)?;
        Ok(VpxFile { compound_file })
    }

    pub fn read_version(&mut self) -> io::Result<Version> {
        read_version(&mut self.compound_file)
    }

    pub fn read_tableinfo(&mut self) -> io::Result<TableInfo> {
        read_tableinfo(&mut self.compound_file)
    }

    pub fn read_gamedata(&mut self) -> io::Result<GameData> {
        let version = self.read_version()?;
        read_gamedata(&mut self.compound_file, &version)
    }

    pub fn read_gameitems(&mut self) -> io::Result<Vec<GameItemEnum>> {
        let gamedata = self.read_gamedata()?;
        read_gameitems(&mut self.compound_file, &gamedata)
    }

    pub fn read_images(&mut self) -> io::Result<Vec<ImageData>> {
        let gamedata = self.read_gamedata()?;
        read_images(&mut self.compound_file, &gamedata)
    }

    pub fn read_sounds(&mut self) -> io::Result<Vec<SoundData>> {
        let version = self.read_version()?;
        let gamedata = self.read_gamedata()?;
        read_sounds(&mut self.compound_file, &gamedata, &version)
    }

    pub fn read_fonts(&mut self) -> io::Result<Vec<FontData>> {
        let gamedata = self.read_gamedata()?;
        read_fonts(&mut self.compound_file, &gamedata)
    }

    pub fn read_collections(&mut self) -> io::Result<Vec<Collection>> {
        let gamedata = self.read_gamedata()?;
        read_collections(&mut self.compound_file, &gamedata)
    }

    pub fn read_custominfotags(&mut self) -> io::Result<CustomInfoTags> {
        read_custominfotags(&mut self.compound_file)
    }

    /// Convert all PNG and BMP images to WebP format and write them back to the VPX file.
    /// This will overwrite the existing images.
    /// The images will be converted to lossless WebP.
    ///
    /// Note: this will not shrink the vpx file, that requires compacting the file.
    ///
    /// Returns a list of conversions that were made.
    pub fn images_to_webp(&mut self) -> io::Result<Vec<ImageToWebpConversion>> {
        // We need to make sure we have read access, or we will get a: Bad file descriptor (os error 9)
        let gamedata = self.read_gamedata()?;
        let results = images_to_webp(&mut self.compound_file, &gamedata)?;
        self.compound_file.flush()?;
        Ok(results)
    }
}

/// Tries to reduce the size of the VPX file by rewriting it.
/// Useful after removing or replacing data in the vpx file
pub fn compact<P: AsRef<Path>>(path: P) -> io::Result<()> {
    compact_cfb(path)
}

/// Rewrites the whole compound file with the same data causing the file to be compacted.
fn compact_cfb<P: AsRef<Path>>(in_path: P) -> io::Result<()> {
    // requested to be added in https://github.com/mdsteele/rust-cfb/issues/55
    let out_path: PathBuf = in_path.as_ref().with_extension("compacting");
    let mut original = cfb::open(&in_path)?;
    let version = original.version();
    let out_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&out_path)?;
    let mut duplicate = CompoundFile::create_with_version(version, out_file)?;
    let mut stream_paths = Vec::<PathBuf>::new();
    for entry in original.walk() {
        if entry.is_storage() {
            if !entry.is_root() {
                duplicate.create_storage(entry.path())?;
            }
            duplicate.set_storage_clsid(entry.path(), *entry.clsid())?;
        } else {
            stream_paths.push(entry.path().to_path_buf());
        }
    }
    for path in stream_paths.iter() {
        std::io::copy(
            &mut original.open_stream(path)?,
            &mut duplicate.create_new_stream(path)?,
        )?;
    }
    duplicate.flush()?;
    std::fs::remove_file(&in_path)?;
    std::fs::rename(&out_path, &in_path)
}

/// Opens a handle to an existing VPX file
pub fn open<P: AsRef<Path>>(path: P) -> io::Result<VpxFile<File>> {
    VpxFile::open(File::open(path)?)
}

pub fn open_rw<P: AsRef<Path>>(path: P) -> io::Result<VpxFile<File>> {
    let file = OpenOptions::new().read(true).write(true).open(path)?;
    VpxFile::open_rw(file)
}

/// Reads a VPX file from disk to memory
///
/// see also [`write()`]
///
/// **Note:** This might take up a lot of memory depending on the size of the VPX file.
pub fn read(path: &Path) -> io::Result<VPX> {
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("File not found: {}", path.display()),
        ));
    }
    let file = File::open(path)?;
    let mut comp = CompoundFile::open_strict(file)?;
    read_vpx(&mut comp)
}

/// Reads a VPX file from bytes to memory
///
/// see also [`write()`]
///
/// **Note:** This might take up a lot of memory depending on the size of the VPX file.
pub fn from_bytes(slice: &[u8]) -> io::Result<VPX> {
    let mut comp = CompoundFile::open_strict(std::io::Cursor::new(slice))?;
    read_vpx(&mut comp)
}

pub fn to_bytes(vpx: &VPX) -> io::Result<Vec<u8>> {
    let buffer = std::io::Cursor::new(Vec::new());
    let mut comp = CompoundFile::create(buffer)?;
    write_vpx(&mut comp, vpx)?;
    comp.flush()?;
    Ok(comp.into_inner().into_inner())
}

/// Writes a VPX file from memory to disk
///
/// see also [`read()`]
pub fn write<P: AsRef<Path>>(path: P, vpx: &VPX) -> io::Result<()> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)?;
    let mut comp = CompoundFile::create(file)?;
    let result = write_vpx(&mut comp, vpx);
    info!(
        "Wrote {}",
        path.as_ref().file_name().unwrap().to_string_lossy()
    );
    result
}

fn read_vpx<F: Read + Seek>(comp: &mut CompoundFile<F>) -> io::Result<VPX> {
    let custominfotags = read_custominfotags(comp)?;
    let info = read_tableinfo(comp)?;
    let version = read_version(comp)?;
    let gamedata = read_gamedata(comp, &version)?;
    let gameitems = read_gameitems(comp, &gamedata)?;
    let images = read_images(comp, &gamedata)?;
    let sounds = read_sounds(comp, &gamedata, &version)?;
    let fonts = read_fonts(comp, &gamedata)?;
    let collections = read_collections(comp, &gamedata)?;
    info!("Loaded VPX");
    Ok(VPX {
        custominfotags,
        info,
        version,
        gamedata,
        gameitems,
        images,
        sounds,
        fonts,
        collections,
    })
}

fn write_vpx<F: Read + Write + Seek>(comp: &mut CompoundFile<F>, vpx: &VPX) -> io::Result<()> {
    create_game_storage(comp)?;
    write_custominfotags(comp, &vpx.custominfotags)?;
    write_tableinfo(comp, &vpx.info)?;
    write_version(comp, &vpx.version)?;
    write_game_data(comp, &vpx.gamedata, &vpx.version)?;
    debug!("Wrote gamedata");
    write_game_items(comp, &vpx.gameitems)?;
    debug!("Wrote {} gameitems", vpx.gameitems.len());
    write_images(comp, &vpx.images)?;
    debug!("Wrote {} images", vpx.images.len());
    write_sounds(comp, &vpx.sounds, &vpx.version)?;
    debug!("Wrote {} sounds", vpx.sounds.len());
    write_fonts(comp, &vpx.fonts)?;
    debug!("Wrote {} fonts", vpx.fonts.len());
    write_collections(comp, &vpx.collections)?;
    debug!("Wrote {} collections", vpx.collections.len());
    let mac = generate_mac(comp)?;
    write_mac(comp, &mac)
}

/// Writes a minimal `vpx` file
pub fn new_minimal_vpx<P: AsRef<Path>>(vpx_file_path: P) -> io::Result<()> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&vpx_file_path)?;
    let mut comp = CompoundFile::create(file)?;
    write_minimal_vpx(&mut comp)
}

fn write_minimal_vpx<F: Read + Write + Seek>(comp: &mut CompoundFile<F>) -> io::Result<()> {
    let table_info = TableInfo::new();
    write_tableinfo(comp, &table_info)?;
    create_game_storage(comp)?;
    let version = Version::new(1072);
    write_version(comp, &version)?;
    write_game_data(comp, &GameData::default(), &version)?;
    // to be more efficient we could generate the mac while writing the different parts
    let mac = generate_mac(comp)?;
    write_mac(comp, &mac)
}

fn create_game_storage<F: Read + Write + Seek>(comp: &mut CompoundFile<F>) -> io::Result<()> {
    let game_stg_path = Path::new(MAIN_SEPARATOR_STR).join("GameStg");
    comp.create_storage(&game_stg_path)
}

/// Extracts the script from an existing `vpx` file.
///
/// # Arguments
/// * `vpx_file_path` Path to the VPX file
/// * `vbs_file_path` Optional path to the script file to write. Defaults to the VPX sidecar script location.
/// * `overwrite` If true, the script will be extracted even if it already exists
pub fn extractvbs(
    vpx_file_path: &Path,
    vbs_file_path: Option<PathBuf>,
    overwrite: bool,
) -> io::Result<ExtractResult> {
    let script_path = match vbs_file_path {
        Some(vbs_file_path) => vbs_file_path,
        None => vbs_path_for(vpx_file_path),
    };

    if !script_path.exists() || (script_path.exists() && overwrite) {
        let mut comp = cfb::open(vpx_file_path)?;
        let version = read_version(&mut comp)?;
        let gamedata = read_gamedata(&mut comp, &version)?;
        extract_script(&gamedata, &script_path)?;
        Ok(ExtractResult::Extracted(script_path))
    } else {
        Ok(ExtractResult::Existed(script_path))
    }
}

/// Imports a script into the provided `vpx` file.
///
/// # Arguments
/// * `vpx_file_path` Path to the VPX file
/// * `vbs_file_path` Optional path to the script file to import. Defaults to the VPX sidecar script location.
///
/// see also [extractvbs]
pub fn importvbs(vpx_file_path: &Path, vbs_file_path: Option<PathBuf>) -> io::Result<PathBuf> {
    let script_path = match vbs_file_path {
        Some(vbs_file_path) => vbs_file_path,
        None => vbs_path_for(vpx_file_path),
    };
    if !script_path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Script file not found: {}", script_path.display()),
        ));
    }
    let mut comp = cfb::open_rw(vpx_file_path)?;
    let version = read_version(&mut comp)?;
    let mut gamedata = read_gamedata(&mut comp, &version)?;
    let script = std::fs::read_to_string(&script_path)?;
    gamedata.set_code(script);
    write_game_data(&mut comp, &gamedata, &version)?;
    let mac = generate_mac(&mut comp)?;
    write_mac(&mut comp, &mac)?;
    comp.flush()?;
    Ok(script_path)
}

/// Verifies the MAC signature of a VPX file
pub fn verify(vpx_file_path: &Path) -> VerifyResult {
    let result = move || -> io::Result<_> {
        let mut comp = cfb::open(vpx_file_path)?;
        let mac = read_mac(&mut comp)?;
        let generated_mac = generate_mac(&mut comp)?;
        Ok((mac, generated_mac))
    }();
    match result {
        Ok((mac, generated_mac)) => {
            if mac == generated_mac {
                VerifyResult::Ok(vpx_file_path.to_path_buf())
            } else {
                VerifyResult::Failed(
                    vpx_file_path.to_path_buf(),
                    format!("MAC mismatch: {mac:?} != {generated_mac:?}"),
                )
            }
        }
        Err(e) => VerifyResult::Failed(
            vpx_file_path.to_path_buf(),
            format!("Failed to read VPX file {}: {}", vpx_file_path.display(), e),
        ),
    }
}

/// Returns the path to the sidecar script for a given `vpx` file
pub fn vbs_path_for(vpx_file_path: &Path) -> PathBuf {
    path_for(vpx_file_path, "vbs")
}

/// Returns the path to table `ini` file
pub fn ini_path_for(vpx_file_path: &Path) -> PathBuf {
    path_for(vpx_file_path, "ini")
}

fn path_for(vpx_file_path: &Path, extension: &str) -> PathBuf {
    PathBuf::from(vpx_file_path).with_extension(extension)
}

fn read_mac<F: Read + Write + Seek>(comp: &mut CompoundFile<F>) -> io::Result<Vec<u8>> {
    let mac_path = Path::new(MAIN_SEPARATOR_STR).join("GameStg").join("MAC");
    if !comp.exists(&mac_path) {
        // fail
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "MAC stream not found",
        ));
    }
    let mut mac_stream = comp.open_stream(mac_path)?;
    let mut mac = Vec::new();
    mac_stream.read_to_end(&mut mac)?;
    Ok(mac)
}

fn write_mac<F: Read + Write + Seek>(comp: &mut CompoundFile<F>, mac: &[u8]) -> io::Result<()> {
    let mac_path = Path::new(MAIN_SEPARATOR_STR).join("GameStg").join("MAC");
    let mut mac_stream = comp.create_stream(mac_path)?;
    mac_stream.write_all(mac)
}

#[derive(Clone, Debug)]
enum FileType {
    UnstructuredBytes,
    Biff,
}

#[derive(Debug)]
struct FileStructureItem {
    path: PathBuf,
    file_type: FileType,
    hashed: bool,
}
// contructor with default values
impl FileStructureItem {
    fn new(path: &str, file_type: FileType, hashed: bool) -> Self {
        FileStructureItem {
            path: PathBuf::from(path),
            file_type,
            hashed,
        }
    }
}

fn generate_mac<F: Read + Seek>(comp: &mut CompoundFile<F>) -> io::Result<Vec<u8>> {
    // Regarding mac generation, see
    //  https://github.com/freezy/VisualPinball.Engine/blob/ec1e9765cd4832c134e889d6e6d03320bc404bd5/VisualPinball.Engine/VPT/Table/TableWriter.cs#L42
    //  https://github.com/vbousquet/vpx_lightmapper/blob/ca5fddd4c2a0fbe817fd546c5f4db609f9d0da9f/addons/vpx_lightmapper/vlm_export.py#L906-L913
    //  https://github.com/vpinball/vpinball/blob/d9d22a5923ad5a9902a27fae296bc6b2e9ed95ca/pintable.cpp#L2634-L2667
    //  ordering of writes is important co come up with the correct hash

    fn item_path(path: &Path, index: i32) -> PathBuf {
        path.with_file_name(format!(
            "{}{}",
            path.file_name().unwrap().to_string_lossy(),
            index
        ))
    }

    fn append_structure<F: Seek + Read>(
        file_structure: &mut Vec<FileStructureItem>,
        comp: &mut CompoundFile<F>,
        src_path: &str,
        file_type: FileType,
        hashed: bool,
    ) {
        let mut index = 0;
        let path = PathBuf::from(src_path);
        while comp.exists(item_path(&path, index)) {
            file_structure.push(FileStructureItem {
                path: item_path(&path, index),
                file_type: file_type.clone(),
                hashed,
            });
            index += 1;
        }
    }

    use FileType::*;

    // above pythin code converted to rust
    let mut file_structure: Vec<FileStructureItem> = vec![
        FileStructureItem::new("GameStg/Version", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/TableName", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/AuthorName", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/TableVersion", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/ReleaseDate", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/AuthorEmail", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/AuthorWebSite", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/TableBlurb", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/TableDescription", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/TableRules", UnstructuredBytes, true),
        FileStructureItem::new("TableInfo/TableSaveDate", UnstructuredBytes, false),
        FileStructureItem::new("TableInfo/TableSaveRev", UnstructuredBytes, false),
        FileStructureItem::new("TableInfo/Screenshot", UnstructuredBytes, true),
        FileStructureItem::new("GameStg/CustomInfoTags", Biff, true), // custom info tags must be hashed just after this stream
        FileStructureItem::new("GameStg/GameData", Biff, true),
    ];
    // //append_structure(&mut file_structure, comp, "GameStg/GameItem", Biff, false);
    //append_structure(&mut file_structure, comp, "GameStg/Sound", Biff, false);
    // //append_structure(&mut file_structure, comp, "GameStg/Image", Biff, false);
    //append_structure(&mut file_structure, comp, "GameStg/Font", Biff, false);
    append_structure(&mut file_structure, comp, "GameStg/Collection", Biff, true);

    let mut hasher = Md2::new();

    // header is always there.
    hasher.update(b"Visual Pinball");

    for item in file_structure {
        if !item.hashed {
            continue;
        }
        if !comp.exists(&item.path) {
            continue;
        }
        match item.file_type {
            UnstructuredBytes => {
                let bytes = read_bytes_at(&item.path, comp)?;
                hasher.update(&bytes);
            }
            Biff => {
                // println!("reading biff: {:?}", item.path);
                let bytes = read_bytes_at(&item.path, comp)?;
                let mut biff = BiffReader::new(&bytes);

                loop {
                    if biff.is_eof() {
                        break;
                    }
                    biff.next(biff::WARN);
                    // println!("reading biff: {:?} {}", item.path, biff.tag());
                    let tag = biff.tag();
                    let tag_str = tag.as_str();
                    match tag_str {
                        "CODE" => {
                            //  For some reason, the code length info is not hashed, just the tag and code string
                            hasher.update(b"CODE");
                            // code is a special case, it indicates a length of 4 (only the tag)
                            // so already 0 bytes remaining
                            let code_length = biff.get_u32_no_remaining_update();
                            let code = biff.get_no_remaining_update(code_length as usize);
                            hasher.update(code);
                        }
                        _other => {
                            // Biff tags and data are hashed but not their size
                            hasher.update(biff.get_record_data(true));
                        }
                    }
                }
            }
        }

        if item.path.ends_with("CustomInfoTags") {
            let bytes = read_bytes_at(&item.path, comp)?;
            let mut biff = BiffReader::new(&bytes);

            loop {
                if biff.is_eof() {
                    break;
                }
                biff.next(biff::WARN);
                if biff.tag() == "CUST" {
                    let cust_name = biff.get_string();
                    //println!("Hashing custom information block {}", cust_name);
                    let path = format!("TableInfo/{cust_name}");
                    if comp.exists(&path) {
                        let data = read_bytes_at(&path, comp)?;
                        hasher.update(&data);
                    }
                } else {
                    biff.skip_tag();
                }
            }
        }
    }
    let result = hasher.finalize();
    Ok(result.to_vec())
}

// TODO this is not very efficient as we copy the bytes around a lot
fn read_bytes_at<F: Read + Seek, P: AsRef<Path>>(
    path: P,
    comp: &mut CompoundFile<F>,
) -> Result<Vec<u8>, io::Error> {
    // println!("reading bytes at: {:?}", path.as_ref());
    let mut bytes = Vec::new();
    let mut stream = comp.open_stream(&path)?;
    stream.read_to_end(&mut bytes).map_err(|e| {
        io::Error::other(
            format!("Failed to read bytes at {:?}, this might be because the file is open in write only mode. {}", path.as_ref(), e),
        )
    })?;
    Ok(bytes)
}

/// Write the script to file in utf8 encoding
pub fn extract_script<P: AsRef<Path>>(gamedata: &GameData, vbs_path: &P) -> Result<(), io::Error> {
    let script = &gamedata.code;
    std::fs::write(vbs_path, &script.string)
}

fn read_gamedata<F: Seek + Read>(
    comp: &mut CompoundFile<F>,
    version: &Version,
) -> io::Result<GameData> {
    let mut game_data_vec = Vec::new();
    let game_data_path = Path::new(MAIN_SEPARATOR_STR)
        .join("GameStg")
        .join("GameData");
    let mut stream = comp.open_stream(game_data_path)?;
    stream.read_to_end(&mut game_data_vec)?;
    let gamedata = gamedata::read_all_gamedata_records(&game_data_vec[..], version);
    Ok(gamedata)
}

fn write_game_data<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    gamedata: &GameData,
    version: &Version,
) -> Result<(), io::Error> {
    let game_data_path = Path::new(MAIN_SEPARATOR_STR)
        .join("GameStg")
        .join("GameData");
    // we expect GameStg to exist
    let mut game_data_stream = comp.create_stream(&game_data_path)?;
    let data = gamedata::write_all_gamedata_records(gamedata, version);
    game_data_stream.write_all(&data)
    // this flush was required before but now it's working without
    // game_data_stream.flush()
}

fn read_gameitems<F: Read + Seek>(
    comp: &mut CompoundFile<F>,
    gamedata: &GameData,
) -> io::Result<Vec<GameItemEnum>> {
    let gamestg = Path::new(MAIN_SEPARATOR_STR).join("GameStg");
    (0..gamedata.gameitems_size)
        .map(|index| {
            let path = gamestg.join(format!("GameItem{index}"));
            let mut input = Vec::new();
            let mut stream = comp.open_stream(&path)?;
            stream.read_to_end(&mut input)?;
            let game_item = gameitem::read(&input);
            Ok(game_item)
        })
        .collect()
}

fn write_game_items<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    gameitems: &[GameItemEnum],
) -> io::Result<()> {
    let gamestg = Path::new(MAIN_SEPARATOR_STR).join("GameStg");
    for (index, gameitem) in gameitems.iter().enumerate() {
        let path = gamestg.join(format!("GameItem{index}"));
        let options = if matches!(gameitem, GameItemEnum::Primitive(_)) {
            CreateStreamOptions::new()
                .buffer_size(64 * 1024)
                .overwrite(false)
        } else {
            CreateStreamOptions::new().overwrite(false)
        };
        let mut stream = comp.create_stream_with_options(&path, options)?;
        let data = gameitem::write(gameitem);
        stream.write_all(&data)?;
    }
    Ok(())
}

fn read_sounds<F: Read + Seek>(
    comp: &mut CompoundFile<F>,
    gamedata: &GameData,
    file_version: &Version,
) -> io::Result<Vec<SoundData>> {
    (0..gamedata.sounds_size)
        .map(|index| {
            let path = Path::new(MAIN_SEPARATOR_STR)
                .join("GameStg")
                .join(format!("Sound{index}"));
            let mut input = Vec::new();
            let options = OpenStreamOptions::new().buffer_size(64 * 1024);
            let mut stream = comp.open_stream_with_options(&path, options)?;
            stream.read_to_end(&mut input)?;
            let mut reader = BiffReader::new(&input);
            let sound = sound::read(file_version, &mut reader);
            Ok(sound)
        })
        .collect()
}

fn write_sounds<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    sounds: &[SoundData],
    file_version: &Version,
) -> io::Result<()> {
    for (index, sound) in sounds.iter().enumerate() {
        let path = Path::new(MAIN_SEPARATOR_STR)
            .join("GameStg")
            .join(format!("Sound{index}"));
        let options = CreateStreamOptions::new()
            .buffer_size(64 * 1024)
            .overwrite(false);
        let mut stream = comp.create_stream_with_options(&path, options)?;
        let mut writer = BiffWriter::new();
        sound::write(file_version, sound, &mut writer);
        stream.write_all(writer.get_data())?;
    }
    Ok(())
}

fn read_collections<F: Read + Seek>(
    comp: &mut CompoundFile<F>,
    gamedata: &GameData,
) -> io::Result<Vec<Collection>> {
    (0..gamedata.collections_size)
        .map(|index| {
            let path = Path::new(MAIN_SEPARATOR_STR)
                .join("GameStg")
                .join(format!("Collection{index}"));
            let mut input = Vec::new();
            let mut stream = comp.open_stream(&path)?;
            stream.read_to_end(&mut input)?;
            Ok(collection::read(&input))
        })
        .collect()
}

fn write_collections<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    collections: &[Collection],
) -> io::Result<()> {
    for (index, collection) in collections.iter().enumerate() {
        let path = Path::new(MAIN_SEPARATOR_STR)
            .join("GameStg")
            .join(format!("Collection{index}"));
        let mut stream = comp.create_stream(&path)?;
        let data = collection::write(collection);
        stream.write_all(&data)?;
    }
    Ok(())
}

fn read_images<F: Read + Seek>(
    comp: &mut CompoundFile<F>,
    gamedata: &GameData,
) -> io::Result<Vec<ImageData>> {
    (0..gamedata.images_size)
        .map(|index| read_image(comp, index))
        .collect()
}

fn read_image<F: Read + Seek>(comp: &mut CompoundFile<F>, index: u32) -> Result<ImageData, Error> {
    let path = format!("GameStg/Image{index}");
    let mut input = Vec::new();
    let options = OpenStreamOptions::new().buffer_size(64 * 1024);
    let mut stream = comp.open_stream_with_options(&path, options)?;
    stream.read_to_end(&mut input)?;
    let mut reader = BiffReader::new(&input);
    Ok(ImageData::biff_read(&mut reader))
}

fn write_images<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    images: &[ImageData],
) -> io::Result<()> {
    for (index, image) in images.iter().enumerate() {
        write_image(comp, index, image, false)?;
    }
    Ok(())
}

fn write_image<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    index: usize,
    image: &ImageData,
    overwrite: bool,
) -> Result<(), Error> {
    let path = format!("GameStg/Image{index}");
    let options = CreateStreamOptions::new()
        .buffer_size(64 * 1024)
        .overwrite(overwrite);
    let mut stream = comp.create_stream_with_options(&path, options)?;
    let mut writer = BiffWriter::new();
    image.biff_write(&mut writer);
    stream.write_all(writer.get_data())?;
    Ok(())
}

#[derive(Debug, PartialEq, Clone)]
pub struct ImageToWebpConversion {
    pub name: String,
    pub old_extension: String,
    pub new_extension: String,
}

fn images_to_webp<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    gamedata: &GameData,
) -> io::Result<Vec<ImageToWebpConversion>> {
    let mut conversions = Vec::new();
    for index in 0..gamedata.images_size {
        let mut image_data = read_image(comp, index)?;
        match image_data.ext().to_lowercase().as_str() {
            "png" => {
                // convert the image to webp
                image_data.change_extension("webp");
                if let Some(jpeg) = &mut image_data.jpeg {
                    // read the image bytes using the rust image library
                    let dynamic_image =
                        match ::image::load_from_memory_with_format(&jpeg.data, ImageFormat::Png) {
                            Ok(image) => image,
                            Err(e) => {
                                // see https://github.com/image-rs/image/issues/2260
                                warn!("Skipping image {}: {}", image_data.name, e);
                                continue;
                            }
                        };

                    // write as webp back to the image
                    let mut webp = Vec::new();
                    let mut cursor = io::Cursor::new(&mut webp);
                    // should be lossless according to the docs
                    dynamic_image
                        .write_to(&mut cursor, ImageFormat::WebP)
                        .map_err(|e| io::Error::other(e.to_string()))?;
                    jpeg.data = webp;
                    write_image(comp, index as usize, &image_data, true)?;
                    conversions.push(ImageToWebpConversion {
                        name: image_data.name.clone(),
                        old_extension: "png".to_string(),
                        new_extension: "webp".to_string(),
                    });
                }
            }
            "bmp" => {
                // convert the image to webp
                image_data.change_extension("webp");
                if let Some(bits) = &mut image_data.bits {
                    // read the image bytes using the rust image library

                    let dynamic_image = vpx_image_to_dynamic_image(
                        &bits.lzw_compressed_data,
                        image_data.width,
                        image_data.height,
                    );

                    // write as webp back to the image
                    let mut webp = Vec::new();
                    let mut cursor = io::Cursor::new(&mut webp);
                    // should be lossless according to the docs
                    dynamic_image
                        .write_to(&mut cursor, ImageFormat::WebP)
                        .map_err(|e| io::Error::other(e.to_string()))?;
                    let jpg = ImageDataJpeg {
                        path: image_data.path.clone(),
                        name: image_data.name.clone(),
                        internal_name: None,
                        data: webp,
                    };
                    image_data.bits = None;
                    image_data.jpeg = Some(jpg);
                    write_image(comp, index as usize, &image_data, true)?;
                }
                conversions.push(ImageToWebpConversion {
                    name: image_data.name.clone(),
                    old_extension: "bmp".to_string(),
                    new_extension: "webp".to_string(),
                });
            }
            _ => {}
        }
    }
    Ok(conversions)
}

fn read_fonts<F: Read + Seek>(
    comp: &mut CompoundFile<F>,
    gamedata: &GameData,
) -> io::Result<Vec<FontData>> {
    (0..gamedata.fonts_size)
        .map(|index| {
            let path = format!("GameStg/Font{index}");
            let mut input = Vec::new();
            let mut stream = comp.open_stream(&path)?;
            stream.read_to_end(&mut input)?;

            let font = font::read(&input);
            Ok(font)
        })
        .collect()
}

fn write_fonts<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    fonts: &[FontData],
) -> io::Result<()> {
    for (index, font) in fonts.iter().enumerate() {
        let path = format!("GameStg/Font{index}");
        let mut stream = comp.create_stream(&path)?;
        let data = font::write(font);
        stream.write_all(&data)?;
    }
    Ok(())
}

fn read_custominfotags<F: Read + Seek>(comp: &mut CompoundFile<F>) -> io::Result<CustomInfoTags> {
    let path = Path::new(MAIN_SEPARATOR_STR)
        .join("GameStg")
        .join("CustomInfoTags");
    let mut tags_data = Vec::new();
    let tags = if comp.is_stream(&path) {
        let mut stream = comp.open_stream(path)?;
        stream.read_to_end(&mut tags_data)?;

        custominfotags::read_custominfotags(&tags_data)
    } else {
        CustomInfoTags::default()
    };
    Ok(tags)
}

fn write_custominfotags<F: Read + Write + Seek>(
    comp: &mut CompoundFile<F>,
    tags: &CustomInfoTags,
) -> io::Result<()> {
    let path = Path::new(MAIN_SEPARATOR_STR)
        .join("GameStg")
        .join("CustomInfoTags");

    let data = custominfotags::write_custominfotags(tags);
    let mut stream = comp.create_stream(path)?;
    stream.write_all(&data)
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_family = "wasm"))]
    use crate::vpx::image::ImageDataBits;
    use pretty_assertions::assert_eq;
    use std::io::Cursor;
    #[cfg(not(target_family = "wasm"))]
    use testdir::testdir;

    use super::*;

    #[test]
    fn test_write_read() -> io::Result<()> {
        let buff = Cursor::new(vec![0; 15]);
        let mut comp = CompoundFile::create(buff)?;
        write_minimal_vpx(&mut comp)?;

        let version = read_version(&mut comp)?;
        let tableinfo = read_tableinfo(&mut comp)?;
        let game_data = read_gamedata(&mut comp, &version)?;

        assert_eq!(tableinfo, TableInfo::new());
        assert_eq!(version, Version::new(1072));
        let expected = GameData::default();
        assert_eq!(game_data, expected);
        Ok(())
    }

    const TEST_TABLE_BYTES: &[u8] =
        include_bytes!("../../testdata/completely_blank_table_10_7_4.vpx");

    #[test]
    fn test_mac_generation() -> io::Result<()> {
        let cursor = Cursor::new(TEST_TABLE_BYTES.to_vec());
        let mut comp = CompoundFile::open_strict(cursor)?;

        let expected = [
            231, 121, 242, 251, 174, 227, 247, 90, 58, 105, 13, 92, 13, 73, 151, 86,
        ];

        let mac = read_mac(&mut comp)?;
        assert_eq!(mac, expected);

        let generated_mac = generate_mac(&mut comp)?;
        assert_eq!(mac, generated_mac);
        Ok(())
    }

    #[test]
    fn test_minimal_mac() -> io::Result<()> {
        let buff = Cursor::new(vec![0; 15]);
        let mut comp = CompoundFile::create(buff)?;
        write_minimal_vpx(&mut comp)?;

        let mac = read_mac(&mut comp)?;
        let expected = [
            162, 17, 22, 72, 167, 156, 25, 141, 150, 149, 231, 8, 65, 201, 152, 225,
        ];
        assert_eq!(mac, expected);
        Ok(())
    }

    #[test]
    fn read_write_gamedata() -> io::Result<()> {
        let cursor = Cursor::new(TEST_TABLE_BYTES.to_vec());
        let mut comp = CompoundFile::open_strict(cursor)?;
        let version = read_version(&mut comp)?;
        let original = read_gamedata(&mut comp, &version)?;

        let buff = Cursor::new(vec![0; 15]);
        let mut comp2 = CompoundFile::create(buff)?;
        create_game_storage(&mut comp2)?;
        write_version(&mut comp2, &version)?;
        write_game_data(&mut comp2, &original, &version)?;

        let read = read_gamedata(&mut comp2, &version)?;

        assert_eq!(original, read);
        Ok(())
    }

    #[test]
    fn read_write_gameitems() -> io::Result<()> {
        let cursor = Cursor::new(TEST_TABLE_BYTES.to_vec());
        let mut comp = CompoundFile::open_strict(cursor)?;
        let version = read_version(&mut comp)?;
        let gamedata = read_gamedata(&mut comp, &version)?;
        let original = read_gameitems(&mut comp, &gamedata)?;

        let buff = Cursor::new(vec![0; 15]);
        let mut comp = CompoundFile::create(buff)?;
        create_game_storage(&mut comp)?;
        write_game_items(&mut comp, &original)?;

        let read = read_gameitems(&mut comp, &gamedata)?;

        assert_eq!(original.len(), read.len());
        assert_eq!(original, read);
        // TODO match original bytes and written bytes for each item
        Ok(())
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn read() -> io::Result<()> {
        let path = PathBuf::from("testdata/completely_blank_table_10_7_4.vpx");
        let mut comp = cfb::open(path)?;
        let original = read_vpx(&mut comp)?;

        let mut expected_info = TableInfo::new();
        expected_info.table_name = Some(String::from("Visual Pinball Demo Table"));
        expected_info.table_save_rev = Some(String::from("10"));
        expected_info.table_version = Some(String::from("1.2"));

        expected_info.author_website = Some(String::from("http://www.vpforums.org/"));
        expected_info.table_save_date = Some(String::from("Tue Jul 11 15:48:49 2023"));
        expected_info.table_description = Some(String::from(
            "Press C to enable manual Ball Control via the arrow keys and B",
        ));

        assert_eq!(original.version, Version::new(1072));
        assert_eq!(original.info, expected_info);
        assert_eq!(original.gamedata.collections_size, 9);
        assert_eq!(original.gamedata.images_size, 1);
        assert_eq!(original.gamedata.sounds_size, 0);
        assert_eq!(original.gamedata.fonts_size, 0);
        assert_eq!(original.gamedata.gameitems_size, 73);
        assert_eq!(original.gameitems.len(), 73);
        assert_eq!(original.images.len(), 1);
        assert_eq!(original.sounds.len(), 0);
        assert_eq!(original.fonts.len(), 0);
        assert_eq!(original.collections.len(), 9);
        Ok(())
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn create_minimal_vpx_and_read() -> io::Result<()> {
        let dir: PathBuf = testdir!();
        let test_vpx_path = dir.join("test.vpx");
        let mut comp = cfb::create(test_vpx_path)?;
        write_minimal_vpx(&mut comp)?;
        comp.flush()?;
        let vpx = read_vpx(&mut comp)?;
        assert_eq!(vpx.info.table_name, None);
        assert_eq!(vpx.info.table_version, None);
        Ok(())
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn images_to_webp_and_compact() -> io::Result<()> {
        let dir: PathBuf = testdir!();
        let test_vpx_path = dir.join("test.vpx");
        let mut vpx = VPX::default();
        // generate random values for the pixels
        let random_pixels = (0..1000 * 1000 * 4)
            .map(|_| rand::random::<u8>())
            .collect::<Vec<u8>>();
        let bmp_image = ImageData {
            name: "bpmimage".to_string(),
            path: "test.bmp".to_string(),
            width: 1000,
            height: 1000,
            bits: Some(ImageDataBits {
                lzw_compressed_data: lzw::to_lzw_blocks(&random_pixels),
            }),
            ..Default::default()
        };
        let dynamic_image = ::image::RgbaImage::from_raw(1000, 1000, random_pixels).unwrap();
        // write the image to a png file in memory
        let mut png_data = Vec::new();
        let mut cursor = io::Cursor::new(&mut png_data);
        dynamic_image
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        let png_image = ImageData {
            name: "pngimage".to_string(),
            path: "test.png".to_string(),
            width: 1000,
            height: 1000,
            jpeg: Some(ImageDataJpeg {
                path: "pngimage".to_string(),
                name: "test.png".to_string(),
                internal_name: None,
                data: png_data,
            }),
            ..Default::default()
        };
        vpx.add_or_replace_image(bmp_image);
        vpx.add_or_replace_image(png_image);
        write(&test_vpx_path, &vpx)?;

        let initial_size = test_vpx_path.metadata()?.len();

        let mut vpx = open_rw(&test_vpx_path)?;
        let updates = vpx.images_to_webp()?;

        compact(&test_vpx_path)?;

        let final_size = test_vpx_path.metadata()?.len();
        assert_eq!(
            updates,
            vec!(
                ImageToWebpConversion {
                    name: "bpmimage".to_string(),
                    old_extension: "bmp".to_string(),
                    new_extension: "webp".to_string(),
                },
                ImageToWebpConversion {
                    name: "pngimage".to_string(),
                    old_extension: "png".to_string(),
                    new_extension: "webp".to_string(),
                },
            )
        );
        println!("Initial size: {initial_size}, Final size: {final_size}");
        assert!(
            final_size < initial_size,
            "Final size: {final_size} >= Initial size: {initial_size}!"
        );

        Ok(())
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_extractvbs_empty_file() {
        let dir: PathBuf = testdir!();
        let test_vpx_path = dir.join("test.vpx");
        // make an empty file
        File::create(&test_vpx_path).unwrap();
        let result = extractvbs(&test_vpx_path, None, false);
        let script_path = vbs_path_for(&test_vpx_path);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid CFB file (0 bytes is too small)",
        );
        assert!(!script_path.exists());
    }

    #[test]
    #[cfg(not(target_family = "wasm"))]
    fn test_verify_empty_file() {
        let dir: PathBuf = testdir!();
        let test_vpx_path = dir.join("test.vpx");
        // make an empty file
        File::create(&test_vpx_path).unwrap();
        let result = verify(&test_vpx_path);
        let script_path = vbs_path_for(&test_vpx_path);
        assert_eq!(
            result,
            VerifyResult::Failed(
                test_vpx_path.clone(),
                format!(
                    "Failed to read VPX file {}: Invalid CFB file (0 bytes is too small)",
                    test_vpx_path.display()
                )
            ),
        );
        assert!(!script_path.exists());
    }
}
