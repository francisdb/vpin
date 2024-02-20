use std::io;
use std::io::{BufWriter, ErrorKind, Write};
use std::path::PathBuf;
use testresult::TestResult;
use vpin::directb2s;
#[test]
fn test() -> TestResult {
    let path = PathBuf::from("testdata/Police Force (Williams 1989) FULL DMD.stripped.directb2s");

    // read file to data
    let file = std::fs::File::open(&path)?;
    let reader = std::io::BufReader::new(file);
    let mut loaded = directb2s::read(reader).map_err(|e| {
        let msg = format!("Error for {}: {}", path.display(), e);
        io::Error::new(ErrorKind::Other, msg)
    })?;

    // strip data
    loaded.strip_images();

    // print as string to console
    let mut buff = String::new();
    directb2s::write(&loaded, &mut buff)?;
    println!("{}", &buff);

    // write buff to file
    let file = std::fs::File::create(path)?;
    let mut writer = BufWriter::new(file);
    writer.write_all(buff.as_bytes())?;

    Ok(())
}
