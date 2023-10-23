use pretty_assertions::assert_eq;
use rayon::prelude::*;
use roxmltree::{Document, Node, NodeType};
use std::collections::hash_map::DefaultHasher;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::io;
use std::io::{Error, ErrorKind, Read};
use std::path::PathBuf;
use testresult::TestResult;
use vpin::directb2s;
use vpin::directb2s::DirectB2SData;

mod common;

#[test]
#[ignore = "slow integration test that only runs on correctly set up machines"]
fn read_all() -> TestResult {
    let home = dirs::home_dir().expect("no home dir");
    let folder = home.join("vpinball").join("tables");
    if !folder.exists() {
        panic!("folder does not exist: {:?}", folder);
    }
    let paths = common::find_files(&folder, "directb2s")?;

    let filtered_file_names = [
        // contains lowercase empty sound[1-9] tags where these should be Capitalized
        "Four Million B.C. (Bally 1971).directb2s",
    ];
    let paths = paths
        .iter()
        .filter(|p| !filtered_file_names.contains(&p.file_name().unwrap().to_str().unwrap()))
        .map(|p| p.to_path_buf())
        .collect::<Vec<_>>();

    paths.par_iter().panic_fuse().try_for_each(|path| {
        println!("testing: {:?}", path);

        // read file to data
        let loaded = read_directb2s(&path)?;

        // write data to buffer
        let mut written = String::new();
        directb2s::write(&loaded, &mut written)?;

        // read original file as xml ast using minidom
        let mut file = std::fs::File::open(&path)?;
        let mut doc = String::new();
        file.read_to_string(&mut doc)?;

        // FIXME workaround for https://github.com/tafia/quick-xml/issues/670
        let mut written = written.replace("\r\n", "&#xD;&#xA;");
        // let original_tail = tail(&doc, 100);
        // let written_tail2 = tail(&written, 100);
        // assert_eq!(original_tail, written_tail2);
        // ---

        let original = roxmltree::Document::parse(&doc)?;

        // read buffer as xml ast
        let written = roxmltree::Document::parse(&mut written)?;

        let original_tree = doc_tree(&original)?;
        let original = format!(
            "{}\n{}",
            path.file_name().unwrap().to_str().unwrap(),
            original_tree
        );
        let written_tree = doc_tree(&written)?;
        let written = format!(
            "{}\n{}",
            path.file_name().unwrap().to_str().unwrap(),
            written_tree
        );

        // compare both
        assert_eq!(original, written);
        Ok(())
    })
}

fn tail(written: &String, n: usize) -> String {
    written
        .chars()
        .rev()
        .take(n)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .to_owned()
}

fn doc_tree(doc: &Document) -> Result<String, std::fmt::Error> {
    let mut writer = String::new();
    doc_to_tag_tree(&doc.root(), "".to_string(), &mut writer)?;
    Ok(writer)
}

fn doc_to_tag_tree<W: Write>(
    node: &Node,
    indent: String,
    writer: &mut W,
) -> Result<(), std::fmt::Error> {
    let t = node.node_type();
    match node.node_type() {
        NodeType::Element => {
            write_node(node, &indent, writer, t)?;
        }
        NodeType::Root => {
            write_node(node, &indent, writer, t)?;
        }
        _ => {
            // skip processing instructions, comments and text
            // println!("skipping: {:?}", t)
        }
    }
    node.children()
        .try_for_each(|child| doc_to_tag_tree(&child, format!("{}  ", indent), writer))
}

fn write_node<W: Write>(
    node: &Node,
    indent: &String,
    writer: &mut W,
    t: NodeType,
) -> Result<(), std::fmt::Error> {
    let mut sorted_attributes = node.attributes().collect::<Vec<_>>();
    sorted_attributes.sort_by_cached_key(|a| a.name());
    let attributes = sorted_attributes
        .iter()
        .map(|a| {
            let value = a.value();
            if value.len() > 100 {
                format!(
                    "{}=hash[{}]{}|{}",
                    a.name(),
                    &value.len(),
                    calculate_hash(&value),
                    &value[0..100]
                )
            } else {
                format!("{}={}", a.name(), a.value())
            }
        })
        .collect::<Vec<_>>();
    let attributes = attributes.join(" ");
    write!(
        writer,
        "{}{:?} {} {}\n",
        indent,
        t,
        node.tag_name().name(),
        attributes
    )?;
    Ok(())
}

fn read_directb2s(path: &PathBuf) -> Result<DirectB2SData, Error> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    directb2s::read(reader).map_err(|e| {
        let msg = format!("Error for {}: {}", path.display(), e);
        io::Error::new(ErrorKind::Other, msg)
    })
}

fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
