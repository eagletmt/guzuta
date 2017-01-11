extern crate crypto;
extern crate lzma;
extern crate rustc_serialize;
extern crate std;
extern crate tar;

use crypto::digest::Digest;
use std::io::Read;

#[derive(Debug, Clone)]
pub struct Package {
    pkginfo: PkgInfo,
    size: u64,
    filename: String,
    pgpsig: String,
    md5sum: String,
    sha256sum: String,
}

impl Package {
    pub fn load<P: AsRef<std::path::Path>>(path: &P) -> Package {
        let pkginfo = PkgInfo::load(path);
        let filename = path.as_ref().file_name().unwrap().to_string_lossy().into_owned();
        let sig_path = path.as_ref().parent().unwrap().join(format!("{}.sig", filename));
        let pgpsig = if let Ok(mut f) = std::fs::File::open(sig_path) {
            use rustc_serialize::base64::ToBase64;

            let mut buf = vec![];
            f.read_to_end(&mut buf).unwrap();
            buf.to_base64(rustc_serialize::base64::STANDARD)
        } else {
            "".to_owned()
        };
        let mut md5 = crypto::md5::Md5::new();
        let mut sha256 = crypto::sha2::Sha256::new();
        let mut f = std::fs::File::open(path).unwrap();
        loop {
            let mut buf = [0; 1024];
            match f.read(&mut buf) {
                Ok(0) => {
                    break;
                }
                Ok(len) => {
                    md5.input(&buf[..len]);
                    sha256.input(&buf[..len]);
                }
                Err(e) => {
                    panic!("{:?}", e);
                }
            }
        }

        Package {
            pkginfo: pkginfo,
            size: std::fs::metadata(path).unwrap().len(),
            filename: filename,
            pgpsig: pgpsig,
            md5sum: md5.result_str(),
            sha256sum: sha256.result_str(),
        }
    }

    pub fn groups(&self) -> &Vec<String> {
        &self.pkginfo.groups
    }
    pub fn license(&self) -> &Vec<String> {
        &self.pkginfo.license
    }
    pub fn replaces(&self) -> &Vec<String> {
        &self.pkginfo.replaces
    }
    pub fn filename(&self) -> &str {
        &self.filename
    }
    pub fn pkgname(&self) -> &str {
        &self.pkginfo.pkgname
    }
    pub fn pkgbase(&self) -> &str {
        &self.pkginfo.pkgbase
    }
    pub fn pkgver(&self) -> &str {
        &self.pkginfo.pkgver
    }
    pub fn pkgdesc(&self) -> &str {
        &self.pkginfo.pkgdesc
    }
    pub fn csize(&self) -> u64 {
        self.size
    }
    pub fn isize(&self) -> u64 {
        self.pkginfo.size
    }
    pub fn md5sum(&self) -> &str {
        &self.md5sum
    }
    pub fn sha256sum(&self) -> &str {
        &self.sha256sum
    }
    pub fn pgpsig(&self) -> &str {
        &self.pgpsig
    }
    pub fn url(&self) -> &str {
        &self.pkginfo.url
    }
    pub fn arch(&self) -> &str {
        &self.pkginfo.arch
    }
    pub fn builddate(&self) -> u64 {
        self.pkginfo.builddate
    }
    pub fn packager(&self) -> &str {
        &self.pkginfo.packager
    }

    pub fn conflicts(&self) -> &Vec<String> {
        &self.pkginfo.conflicts
    }
    pub fn provides(&self) -> &Vec<String> {
        &self.pkginfo.provides
    }
    pub fn depends(&self) -> &Vec<String> {
        &self.pkginfo.depends
    }
    pub fn makedepends(&self) -> &Vec<String> {
        &self.pkginfo.makedepends
    }
    pub fn checkdepends(&self) -> &Vec<String> {
        &self.pkginfo.checkdepends
    }
    pub fn optdepends(&self) -> &Vec<String> {
        &self.pkginfo.optdepends
    }
}

#[derive(Debug, Default, Clone)]
pub struct PkgInfo {
    pub pkgname: String,
    pub pkgbase: String,
    pub pkgver: String,
    pub pkgdesc: String,
    pub url: String,
    pub builddate: u64,
    pub packager: String,
    pub size: u64,
    pub arch: String,
    pub license: Vec<String>,
    pub groups: Vec<String>,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    pub checkdepends: Vec<String>,
    pub optdepends: Vec<String>,
    pub conflicts: Vec<String>,
    pub provides: Vec<String>,
    pub replaces: Vec<String>,
}

impl PkgInfo {
    fn load<P: AsRef<std::path::Path>>(path: &P) -> Self {
        let file = std::fs::File::open(path).unwrap();
        let xz_reader = lzma::LzmaReader::new_decompressor(file).unwrap();
        let mut tar_reader = tar::Archive::new(xz_reader);
        for entry_result in tar_reader.entries().unwrap() {
            let mut entry = entry_result.unwrap();
            if entry.path().unwrap() == std::path::Path::new(".PKGINFO") &&
               entry.header().entry_type() == tar::EntryType::Regular {
                let mut body = String::new();
                entry.read_to_string(&mut body).unwrap();
                return parse_pkginfo(&body);
            }
        }
        panic!(".PKGINFO not found");
    }
}

fn parse_pkginfo(body: &str) -> PkgInfo {
    let mut info = PkgInfo::default();
    for line in body.lines() {
        if line.starts_with('#') {
            continue;
        }
        let splitn: Vec<&str> = line.splitn(2, '=').collect();
        if splitn.len() == 2 {
            let key = splitn[0].trim();
            let val = splitn[1].trim();
            match key {
                "pkgname" => info.pkgname = val.to_owned(),
                "pkgbase" => info.pkgbase = val.to_owned(),
                "pkgver" => info.pkgver = val.to_owned(),
                "pkgdesc" => info.pkgdesc = val.to_owned(),
                "url" => info.url = val.to_owned(),
                "builddate" => info.builddate = val.parse().unwrap(),
                "packager" => info.packager = val.to_owned(),
                "size" => info.size = val.parse().unwrap(),
                "arch" => info.arch = val.to_owned(),
                "license" => info.license.push(val.to_owned()),
                "group" => info.groups.push(val.to_owned()),
                "depend" => info.depends.push(val.to_owned()),
                "makedepend" => info.makedepends.push(val.to_owned()),
                "checkdepend" => info.checkdepends.push(val.to_owned()),
                "optdepend" => info.optdepends.push(val.to_owned()),
                "conflict" => info.conflicts.push(val.to_owned()),
                "provides" => info.provides.push(val.to_owned()),
                "replaces" => info.replaces.push(val.to_owned()),
                _ => panic!("Unknown PKGINFO entry '{}': {}", key, line),
            }
        } else {
            panic!("Invalid line: {}", line);
        }
    }
    return info;
}
