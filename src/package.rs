extern crate base64;
extern crate crypto;
extern crate lzma;
extern crate std;
extern crate tar;

use crypto::digest::Digest;
use std::io::Read;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Lzma(lzma::LzmaError),
    ParseInt(std::num::ParseIntError),
    Custom(std::borrow::Cow<'static, str>),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<lzma::LzmaError> for Error {
    fn from(e: lzma::LzmaError) -> Self {
        Error::Lzma(e)
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Self {
        Error::ParseInt(e)
    }
}

impl From<&'static str> for Error {
    fn from(e: &'static str) -> Self {
        Error::Custom(std::borrow::Cow::from(e))
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::Custom(std::borrow::Cow::from(e))
    }
}

#[derive(Debug, Clone)]
pub struct Package {
    pkginfo: PkgInfo,
    size: u64,
    filename: std::ffi::OsString,
    pgpsig: String,
    md5sum: String,
    sha256sum: String,
    files: Vec<std::path::PathBuf>,
}

impl Package {
    pub fn load<P>(path: P) -> Result<Package, Error>
    where
        P: AsRef<std::path::Path>,
    {
        let path = path.as_ref();
        let (pkginfo, files) = try!(PkgInfo::load(path));
        let mut sig_path = path.as_os_str().to_os_string();
        sig_path.push(".sig");
        let pgpsig = if let Ok(mut f) = std::fs::File::open(sig_path) {
            let mut buf = vec![];
            try!(f.read_to_end(&mut buf));
            base64::encode(&buf)
        } else {
            "".to_owned()
        };
        let mut md5 = crypto::md5::Md5::new();
        let mut sha256 = crypto::sha2::Sha256::new();
        let mut f = try!(std::fs::File::open(path));
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
                    return Err(Error::from(e));
                }
            }
        }

        Ok(Package {
            pkginfo: pkginfo,
            size: try!(std::fs::metadata(path)).len(),
            filename: path.file_name()
                .expect("Unable to find file_name from package path")
                .to_os_string(),
            pgpsig: pgpsig,
            md5sum: md5.result_str(),
            sha256sum: sha256.result_str(),
            files: files,
        })
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
    pub fn filename(&self) -> &std::ffi::OsStr {
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
    pub fn backups(&self) -> &Vec<String> {
        &self.pkginfo.backups
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

    pub fn files(&self) -> &Vec<std::path::PathBuf> {
        &self.files
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
    pub backups: Vec<String>,
    pub provides: Vec<String>,
    pub replaces: Vec<String>,
}

impl PkgInfo {
    fn load<P>(path: P) -> Result<(Self, Vec<std::path::PathBuf>), Error>
    where
        P: AsRef<std::path::Path>,
    {
        let file = try!(std::fs::File::open(path));
        let xz_reader = try!(lzma::LzmaReader::new_decompressor(file));
        let mut tar_reader = tar::Archive::new(xz_reader);
        let mut pkginfo = None;
        let mut files = vec![];
        for entry_result in try!(tar_reader.entries()) {
            let mut entry = try!(entry_result);
            let path = try!(entry.path()).into_owned();
            if path.as_os_str() == ".PKGINFO"
                && entry.header().entry_type() == tar::EntryType::Regular
            {
                let mut body = String::new();
                try!(entry.read_to_string(&mut body));
                pkginfo = Some(try!(parse_pkginfo(&body)));
            }
            if !path.starts_with(".") {
                files.push(path.to_path_buf());
            }
        }
        if let Some(pkginfo) = pkginfo {
            Ok((pkginfo, files))
        } else {
            Err(Error::from(".PKGINFO not found"))
        }
    }
}

fn parse_pkginfo(body: &str) -> Result<PkgInfo, Error> {
    let mut info = PkgInfo::default();
    for line in body.lines() {
        if line.starts_with('#') {
            continue;
        }
        let mut splitn = line.splitn(2, '=');
        let key = splitn.next();
        let val = splitn.next();
        let rest = splitn.next();
        if let (Some(key), Some(val), None) = (key, val, rest) {
            let key = key.trim();
            let val = val.trim();
            match key {
                "pkgname" => info.pkgname = val.to_owned(),
                "pkgbase" => info.pkgbase = val.to_owned(),
                "pkgver" => info.pkgver = val.to_owned(),
                "pkgdesc" => info.pkgdesc = val.to_owned(),
                "url" => info.url = val.to_owned(),
                "builddate" => info.builddate = try!(val.parse()),
                "packager" => info.packager = val.to_owned(),
                "size" => info.size = try!(val.parse()),
                "arch" => info.arch = val.to_owned(),
                "license" => info.license.push(val.to_owned()),
                "group" => info.groups.push(val.to_owned()),
                "depend" => info.depends.push(val.to_owned()),
                "makedepend" => info.makedepends.push(val.to_owned()),
                "checkdepend" => info.checkdepends.push(val.to_owned()),
                "optdepend" => info.optdepends.push(val.to_owned()),
                "conflict" => info.conflicts.push(val.to_owned()),
                "provides" => info.provides.push(val.to_owned()),
                "backup" => info.backups.push(val.to_owned()),
                "replaces" => info.replaces.push(val.to_owned()),
                _ => {
                    return Err(Error::from(format!(
                        "Unknown PKGINFO entry '{}': {}",
                        key, line
                    )))
                }
            }
        } else {
            return Err(Error::from(format!("Invalid line: {}", line)));
        }
    }
    Ok(info)
}
