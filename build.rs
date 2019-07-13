use std::env;
use std::io::Write;
use std::fs::File;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=build.rs,openings.tsv");

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_path("openings.tsv").unwrap();

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("opening_db.rs");
    let mut f = File::create(&dest_path).unwrap();

    write!(&mut f, "static OPENING_DB: phf::Map<&'static str, Opening>= ").unwrap();
    let mut map = phf_codegen::Map::new();
    for line in reader.records() {
        let (epd, record) = {
            let record = line.unwrap();
            let eco = record.get(0).unwrap();
            let name = record.get(1).unwrap();
            let epd = record.get(2).unwrap();
            (epd.to_owned(), format!(r#"Opening {{ eco: "{}", name: "{}" }}"#, eco, name))
        };
        map.entry(epd, record.as_str());
    }
    map.build(&mut f).unwrap();
    write!(&mut f, ";\n").unwrap();
}
