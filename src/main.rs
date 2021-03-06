use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write as _;

use clap::App;
use clap::Arg;

mod main_logic;

fn main() {
    let m = App::new("Duplicated Files Detector")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .arg(
            Arg::with_name("root")
                .value_name("PATH")
                .help("The root directory of scanning")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ignore")
                .value_name("IGNORE-GLOB")
                .long("ignore")
                .help("The globs to specify ignoring files or directories")
                .takes_value(true)
                .multiple(true)
                .validator(|x| {
                    glob::Pattern::new(&x)
                        .map(|_| ())
                        .map_err(|e| format!("{}", e))
                }),
        )
        .get_matches();

    let root = m.value_of("root").unwrap();

    let ignore = m.values_of("ignore");
    let ignore_globs = ignore
        .map(|xs| {
            xs.into_iter()
                .map(|x| glob::Pattern::new(x).unwrap())
                .collect()
        })
        .unwrap_or(Vec::new());

    let start_time = std::time::SystemTime::now();

    let mut collection = HashMap::<_, Vec<_>>::new();
    let _ = main_logic::scan(
        root,
        |path| {
            let is_dir = path.is_dir();
            if let Some(path) = path.to_str() {
                let mut path = path.to_string();
                if is_dir {
                    path.push(std::path::MAIN_SEPARATOR);
                }
                ignore_globs.iter().any(|g| g.matches(&path))
            } else {
                false
            }
        },
        |path, hash| {
            collection
                .entry(hash.to_vec())
                .or_default()
                .push(path.to_path_buf())
        },
        |path, err| println!("{:?}: {}", err, path.display()),
    );

    let t = start_time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let _ = std::fs::create_dir("./outputs");
    let mut f = BufWriter::new(File::create(format!("./outputs/{}.txt", t)).unwrap());

    let mut any_dup = false;
    for pair in &collection {
        if pair.1.len() >= 2 {
            any_dup = true;
            println!("{}", bytes_to_hex(pair.0));
            writeln!(f, "{}", bytes_to_hex(pair.0))
                .expect("!!! Failed to write into the output file !!!");
            for entry in pair.1 {
                println!("    {}", entry.display());
                writeln!(f, "    {}", entry.display())
                    .expect("!!! Failed to write into the output file !!!");
            }
        }
    }
    if !any_dup {
        println!("Non of Duplicated Entries");
    }
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::new();
    for byte in bytes {
        write!(s, "{:02X}", byte).unwrap();
    }
    s
}
