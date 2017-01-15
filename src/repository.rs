extern crate flate2;
extern crate gpgme;
extern crate std;
extern crate tar;

use std::io::Read;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Gpgme(gpgme::Error),
    ParseInt(std::num::ParseIntError),
    Custom(std::borrow::Cow<'static, str>),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<super::signer::Error> for Error {
    fn from(e: super::signer::Error) -> Self {
        match e {
            super::signer::Error::Gpgme(e) => Error::Gpgme(e),
            super::signer::Error::Io(e) => Error::Io(e),
        }
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Self {
        Error::ParseInt(e)
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::Custom(std::borrow::Cow::from(e))
    }
}

impl From<&'static str> for Error {
    fn from(e: &'static str) -> Self {
        Error::Custom(std::borrow::Cow::from(e))
    }
}

#[derive(Debug, Default, Clone)]
pub struct Desc {
    groups: Vec<String>,
    license: Vec<String>,
    replaces: Vec<String>,
    filename: String,
    name: String,
    base: String,
    version: String,
    desc: String,
    csize: u64,
    isize: u64,
    md5sum: String,
    sha256sum: String,
    pgpsig: String,
    url: String,
    arch: String,
    builddate: u64,
    packager: String,

    // These fields are once stored in depends file
    // https://git.archlinux.org/pacman.git/commit/?id=b520c6312ff0ffec864576b5218f1638fae1e18b
    conflicts: Vec<String>,
    provides: Vec<String>,
    depends: Vec<String>,
    makedepends: Vec<String>,
    checkdepends: Vec<String>,
    optdepends: Vec<String>,
}

#[derive(Clone)]
pub struct PackageEntry {
    pub desc: Desc,
    pub files: Vec<String>,
}

#[derive(Clone)]
pub struct Repository<'a> {
    path: std::path::PathBuf,
    signer: Option<&'a super::signer::Signer<'a>>,
    entries: std::collections::HashMap<String, PackageEntry>,
}

impl<'a> Repository<'a> {
    pub fn new(path: std::path::PathBuf,
               signer: Option<&'a super::signer::Signer<'a>>)
               -> Repository {
        return Repository {
            path: path,
            signer: signer,
            entries: std::collections::HashMap::new(),
        };
    }

    pub fn load(&mut self) -> Result<(), Error> {
        match std::fs::File::open(&self.path) {
            Ok(file) => self.load_from_file(file),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(Error::from(e))
                }
            }
        }
    }

    fn load_from_file(&mut self, file: std::fs::File) -> Result<(), Error> {
        let gz_reader = try!(flate2::read::GzDecoder::new(file));
        let mut tar_reader = tar::Archive::new(gz_reader);
        let mut desc_entries = std::collections::HashMap::new();
        let mut files_entries = std::collections::HashMap::new();
        for entry_result in try!(tar_reader.entries()) {
            let mut entry = try!(entry_result);
            let pathbuf = try!(entry.path()).into_owned();
            let pathname = pathbuf.to_str().unwrap();
            match entry.header().entry_type() {
                tar::EntryType::Regular => {
                    let splitn: Vec<&str> = pathname.splitn(2, '/').collect();
                    if splitn.len() == 2 {
                        let mut body = String::new();
                        try!(entry.read_to_string(&mut body));
                        match splitn[1] {
                            "desc" => {
                                desc_entries.insert(splitn[0].to_owned(), try!(parse_desc(&body)));
                            }
                            "depends" => {
                                // old format
                            }
                            "files" => {
                                files_entries.insert(splitn[0].to_owned(), try!(parse_files(&body)));
                            }
                            _ => {
                                return Err(Error::from(format!("Unknown pathname: {}", pathname)));
                            }
                        }
                    } else {
                        return Err(Error::from(format!("Invalid pathname entry: {}", pathname)));
                    }
                }
                tar::EntryType::Directory => {}
                _ => {
                    return Err(Error::from(format!("Unknown file type: {}", pathname)));
                }
            }
        }

        for (_, desc) in desc_entries.into_iter() {
            let files = files_entries.remove(&desc.name).unwrap_or(vec![]);
            self.entries.insert(desc.name.to_owned(),
                                PackageEntry {
                                    desc: desc,
                                    files: files,
                                });
        }
        Ok(())
    }

    pub fn add(&mut self, package: &super::package::Package) {
        let desc = Desc {
            groups: package.groups().to_owned(),
            license: package.license().to_owned(),
            replaces: package.replaces().to_owned(),
            filename: package.filename().to_owned(),
            name: package.pkgname().to_owned(),
            base: package.pkgbase().to_owned(),
            version: package.pkgver().to_owned(),
            desc: package.pkgdesc().to_owned(),
            csize: package.csize(),
            isize: package.isize(),
            md5sum: package.md5sum().to_owned(),
            sha256sum: package.sha256sum().to_owned(),
            pgpsig: package.pgpsig().to_owned(),
            url: package.url().to_owned(),
            arch: package.arch().to_owned(),
            builddate: package.builddate(),
            packager: package.packager().to_owned(),
            conflicts: package.conflicts().to_owned(),
            provides: package.provides().to_owned(),
            depends: package.depends().to_owned(),
            makedepends: package.makedepends().to_owned(),
            checkdepends: package.checkdepends().to_owned(),
            optdepends: package.optdepends().to_owned(),
        };
        self.entries.insert(desc.name.to_owned(),
                            PackageEntry {
                                desc: desc,
                                files: package.files().to_owned(),
                            });
    }

    pub fn remove(&mut self, package_name: &str) {
        self.entries.remove(package_name);
    }

    pub fn save(&self, include_files: bool) -> Result<(), Error> {
        let mut tmp_path = self.path.clone().into_os_string();
        tmp_path.push(".progress");
        let file = try!(std::fs::File::create(&tmp_path));
        let gz_writer = flate2::write::GzEncoder::new(file, flate2::Compression::Default);
        let mut builder = tar::Builder::new(gz_writer);
        for (_, package_entry) in self.entries.iter() {
            let pathbuf = std::path::PathBuf::from(format!("{}-{}/",
                                                           package_entry.desc.name,
                                                           package_entry.desc.version));
            {
                let mut dir_header = tar::Header::new_gnu();
                dir_header.set_entry_type(tar::EntryType::Directory);
                try!(dir_header.set_path(&pathbuf));
                dir_header.set_mode(0o755);
                dir_header.set_size(0);
                dir_header.set_cksum();
                try!(builder.append(&dir_header, std::io::empty()));
            }
            {
                let mut desc_header = tar::Header::new_gnu();
                desc_header.set_entry_type(tar::EntryType::Regular);
                try!(desc_header.set_path(pathbuf.join("desc")));
                desc_header.set_mode(0o644);
                let desc_str = into_desc_file(package_entry);
                let desc_bytes = desc_str.as_bytes();
                desc_header.set_size(desc_bytes.len() as u64);
                desc_header.set_cksum();
                try!(builder.append(&desc_header, desc_bytes));
            }
            if include_files {
                let mut files_header = tar::Header::new_gnu();
                files_header.set_entry_type(tar::EntryType::Regular);
                try!(files_header.set_path(pathbuf.join("files")));
                files_header.set_mode(0o644);
                let files_str = into_files_file(&package_entry.files);
                let files_bytes = files_str.as_bytes();
                files_header.set_size(files_bytes.len() as u64);
                files_header.set_cksum();
                try!(builder.append(&files_header, files_bytes));
            }
        }
        let gz_writer = try!(builder.into_inner());
        try!(gz_writer.finish());

        if let Some(ref signer) = self.signer {
            let mut sig_path = self.path.clone().into_os_string();
            sig_path.push(".sig");
            try!(signer.sign(&tmp_path, sig_path));
        }

        try!(std::fs::rename(&tmp_path, &self.path));
        Ok(())
    }
}

fn parse_desc(body: &str) -> Result<Desc, Error> {
    let mut desc = Desc::default();
    for (key, val) in each_entry(body) {
        match key {
            "GROUPS" => {
                desc.groups.push(val.to_owned());
            }
            "REPLACES" => {
                desc.replaces.push(val.to_owned());
            }
            "FILENAME" => {
                desc.filename = val.to_owned();
            }
            "NAME" => {
                desc.name = val.to_owned();
            }
            "BASE" => {
                desc.base = val.to_owned();
            }
            "VERSION" => {
                desc.version = val.to_owned();
            }
            "DESC" => {
                desc.desc = val.to_owned();
            }
            "CSIZE" => {
                desc.csize = try!(val.parse());
            }
            "ISIZE" => {
                desc.isize = try!(val.parse());
            }
            "MD5SUM" => {
                desc.md5sum = val.to_owned();
            }
            "SHA256SUM" => {
                desc.sha256sum = val.to_owned();
            }
            "PGPSIG" => {
                desc.pgpsig = val.to_owned();
            }
            "URL" => {
                desc.url = val.to_owned();
            }
            "LICENSE" => {
                desc.license.push(val.to_owned());
            }
            "ARCH" => {
                desc.arch = val.to_owned();
            }
            "BUILDDATE" => {
                desc.builddate = try!(val.parse());
            }
            "PACKAGER" => {
                desc.packager = val.to_owned();
            }
            "CONFLICTS" => {
                desc.conflicts.push(val.to_owned());
            }
            "PROVIDES" => {
                desc.provides.push(val.to_owned());
            }
            "DEPENDS" => {
                desc.depends.push(val.to_owned());
            }
            "MAKEDEPENDS" => {
                desc.makedepends.push(val.to_owned());
            }
            "CHECKDEPENDS" => {
                desc.checkdepends.push(val.to_owned());
            }
            "OPTDEPENDS" => {
                desc.optdepends.push(val.to_owned());
            }
            _ => {
                return Err(Error::from(format!("Unknown desc entry: {}", key)));
            }
        }
    }
    Ok(desc)
}

struct EachEntry<'a> {
    key: &'a str,
    lines: std::str::Lines<'a>,
}

impl<'a> Iterator for EachEntry<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(line) = self.lines.next() {
            let line = line.trim();
            if line.starts_with('%') && line.ends_with('%') {
                self.key = &line[1..line.len() - 1];
            } else if line.is_empty() {
                self.key = "";
            } else {
                return Some((self.key, line));
            }
        }
        return None;
    }
}

fn each_entry(body: &str) -> EachEntry {
    EachEntry {
        key: "",
        lines: body.lines(),
    }
}

fn into_desc_file(package_entry: &PackageEntry) -> String {
    let mut buf = String::new();
    let ref desc = package_entry.desc;
    desc_write_array(&mut buf, "GROUPS", &desc.groups);
    desc_write_array(&mut buf, "REPLACES", &desc.replaces);
    desc_write_string(&mut buf, "FILENAME", &desc.filename);
    desc_write_string(&mut buf, "NAME", &desc.name);
    desc_write_string(&mut buf, "BASE", &desc.base);
    desc_write_string(&mut buf, "VERSION", &desc.version);
    desc_write_string(&mut buf, "DESC", &desc.desc);
    desc_write_u64(&mut buf, "CSIZE", desc.csize);
    desc_write_u64(&mut buf, "ISIZE", desc.isize);
    desc_write_string(&mut buf, "MD5SUM", &desc.md5sum);
    desc_write_string(&mut buf, "SHA256SUM", &desc.sha256sum);
    desc_write_string(&mut buf, "PGPSIG", &desc.pgpsig);
    desc_write_string(&mut buf, "URL", &desc.url);
    desc_write_array(&mut buf, "LICENSE", &desc.license);
    desc_write_string(&mut buf, "ARCH", &desc.arch);
    desc_write_u64(&mut buf, "BUILDDATE", desc.builddate);
    desc_write_string(&mut buf, "PACKAGER", &desc.packager);
    desc_write_array(&mut buf, "CONFLICTS", &desc.conflicts);
    desc_write_array(&mut buf, "PROVIDES", &desc.provides);
    desc_write_array(&mut buf, "DEPENDS", &desc.depends);
    desc_write_array(&mut buf, "MAKEDEPENDS", &desc.makedepends);
    desc_write_array(&mut buf, "CHECKDEPENDS", &desc.checkdepends);
    desc_write_array(&mut buf, "OPTDEPENDS", &desc.optdepends);
    return buf;
}

fn desc_write_array(buf: &mut String, key: &str, xs: &Vec<String>) {
    if !xs.is_empty() {
        buf.push_str("%");
        buf.push_str(key);
        buf.push_str("%\n");
        for x in xs {
            buf.push_str(&x);
            buf.push_str("\n");
        }
        buf.push_str("\n");
    }
}

fn desc_write_string(buf: &mut String, key: &str, val: &str) {
    if !val.is_empty() {
        buf.push_str("%");
        buf.push_str(key);
        buf.push_str("%\n");
        buf.push_str(val);
        buf.push_str("\n\n");
    }
}

fn desc_write_u64(buf: &mut String, key: &str, val: u64) {
    if val != 0 {
        buf.push_str("%");
        buf.push_str(key);
        buf.push_str("%\n");
        buf.push_str(&format!("{}", val));
        buf.push_str("\n\n");
    }
}

fn parse_files(body: &str) -> Result<Vec<String>, Error> {
    let mut iter = body.lines();

    if let Some("%FILES%") = iter.next() {
        let mut files = vec![];
        for line in iter {
            files.push(line.to_owned());
        }
        Ok(files)
    } else {
        Err(Error::from("Empty files file"))
    }
}

fn into_files_file(files: &Vec<String>) -> String {
    let mut buf = String::new();
    buf.push_str("%FILES%\n");
    for file in files {
        buf.push_str(file);
        buf.push_str("\n");
    }
    return buf;
}
