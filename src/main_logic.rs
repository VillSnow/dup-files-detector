use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

#[derive(Debug)]
pub enum Error {
    IOError(std::io::Error),
    EncodeError,
    Ignore,
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::IOError(e)
    }
}

pub fn scan(
    path: impl AsRef<Path>,
    ignore_pred: impl FnMut(&Path) -> bool,
    callback: impl FnMut(&Path, &[u8]),
    error_callback: impl FnMut(&Path, &Error),
) -> Result<Vec<u8>, Error> {
    scan_impl(
        path.as_ref(),
        &RefCell::new(ignore_pred),
        &RefCell::new(callback),
        &RefCell::new(error_callback),
    )
}

pub fn scan_impl(
    node: &Path,
    ignore_pred: &RefCell<impl FnMut(&Path) -> bool>,
    callback: &RefCell<impl FnMut(&Path, &[u8])>,
    error_callback: &RefCell<impl FnMut(&Path, &Error)>,
) -> Result<Vec<u8>, Error> {
    eprintln!("{}", node.display());
    let node = node.as_ref();

    if ignore_pred.borrow_mut()(node) {
        return Err(Error::Ignore);
    }

    let md = fs::symlink_metadata(node)?;
    let fty = md.file_type();

    let result = if fty.is_file() {
        file_hash(node)
    } else if fty.is_dir() {
        scan_dir(node, ignore_pred, callback, error_callback)
    } else if fty.is_symlink() {
        symlink_hash(node)
    } else {
        panic!(format!("{:?}", md))
    };

    match result {
        Ok(hash) => {
            callback.borrow_mut()(node, &hash);
            return Ok(hash);
        }
        Err(err) => {
            error_callback.borrow_mut()(node, &err);
            return Err(err);
        }
    }
}

fn scan_dir(
    path: &Path,
    ignore_pred: &RefCell<impl FnMut(&Path) -> bool>,
    callback: &RefCell<impl FnMut(&Path, &[u8])>,
    error_callback: &RefCell<impl FnMut(&Path, &Error)>,
) -> Result<Vec<u8>, Error> {
    let iter = fs::read_dir(path)?;
    let mut children = Vec::new();
    for entry in iter {
        let entry = entry?;
        let path = entry.path();

        if ignore_pred.borrow_mut()(&path) {
            continue;
        }

        let hash = scan_impl(&path, ignore_pred, callback, error_callback)?;
        children.push((path.file_name().unwrap().to_os_string(), hash));
    }

    dir_hash(children.iter().map(|(a, b)| (a.as_os_str(), b)))
}

fn file_hash(path: &Path) -> Result<Vec<u8>, Error> {
    let mut hasher = Sha256::new();

    let mut f = File::open(path)?;
    let mut buf = vec![0; 2048];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf);
    }

    return Ok(hasher.finalize().to_vec());
}

fn dir_hash<'a, I>(children: I) -> Result<Vec<u8>, Error>
where
    I: IntoIterator<Item = (&'a OsStr, &'a Vec<u8>)>,
{
    let mut children = children
        .into_iter()
        .map(|entry| -> Result<_, Error> {
            let name = entry.0;
            let name = name.to_str().ok_or(Error::EncodeError)?;
            let hash = entry.1;
            Ok((name, hash))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let unique = children.iter().map(|x| x.0).collect::<HashSet<_>>();
    if unique.len() != children.len() {
        panic!("!!! Duplicated entry name !!!");
    }

    children.sort_by_key(|x| x.0);

    let mut hasher = Sha256::new();
    let salt = "6aIecn4M7VoB";
    hasher.update(salt.as_bytes());

    for entry in children {
        let name = entry.0;
        let hash = entry.1;
        hasher.update(name.as_bytes());
        hasher.update(hash);
    }

    Ok(hasher.finalize().to_vec())
}

fn symlink_hash(path: &Path) -> Result<Vec<u8>, Error> {
    let link = fs::read_link(path)?;
    eprintln!("{} -> {}", path.display(), link.display());
    let link = link.to_str().ok_or(Error::EncodeError)?;

    let mut hasher = Sha256::new();
    let salt = "RXqENRdyGIpE";
    hasher.update(salt.as_bytes());
    hasher.update(link.as_bytes());
    Ok(hasher.finalize().to_vec())
}
