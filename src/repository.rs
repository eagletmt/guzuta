extern crate failure;
extern crate flate2;
extern crate gpgme;
extern crate std;
extern crate tar;

use std::io::Read;

#[derive(Debug, Default, Clone)]
pub struct Desc {
    groups: Vec<String>,
    license: Vec<String>,
    replaces: Vec<String>,
    filename: std::ffi::OsString,
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
    backups: Vec<String>,
    depends: Vec<String>,
    makedepends: Vec<String>,
    checkdepends: Vec<String>,
    optdepends: Vec<String>,
}

#[derive(Clone)]
pub struct PackageEntry {
    pub desc: Desc,
    pub files: Vec<std::path::PathBuf>,
}

#[derive(Clone)]
pub struct Repository<'a> {
    path: std::path::PathBuf,
    signer: Option<&'a super::signer::Signer<'a>>,
    entries: std::collections::HashMap<String, PackageEntry>,
}

impl<'a> Repository<'a> {
    pub fn new(
        path: std::path::PathBuf,
        signer: Option<&'a super::signer::Signer<'a>>,
    ) -> Repository {
        Repository {
            path,
            signer,
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn path(&self) -> &std::path::Path {
        self.path.as_path()
    }

    pub fn load(&mut self) -> Result<(), failure::Error> {
        match std::fs::File::open(&self.path) {
            Ok(file) => self.load_from_file(file),
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(failure::Error::from(e))
                }
            }
        }
    }

    fn load_from_file(&mut self, file: std::fs::File) -> Result<(), failure::Error> {
        let gz_reader = flate2::read::GzDecoder::new(file);
        let mut tar_reader = tar::Archive::new(gz_reader);
        let mut desc_entries = std::collections::HashMap::new();
        let mut files_entries = std::collections::HashMap::new();
        for entry_result in tar_reader.entries()? {
            let mut entry = entry_result?;
            let pathbuf = entry.path()?.into_owned();
            let pathname = pathbuf.to_str().expect("Unable to convert PathBuf to str");
            match entry.header().entry_type() {
                tar::EntryType::Regular => {
                    let mut splitn = pathname.splitn(2, '/');
                    let pkgname = splitn.next();
                    let filename = splitn.next();
                    let rest = splitn.next();
                    if let (Some(pkgname), Some(filename), None) = (pkgname, filename, rest) {
                        let mut body = String::new();
                        entry.read_to_string(&mut body)?;
                        match filename {
                            "desc" => {
                                desc_entries.insert(pkgname.to_owned(), parse_desc(&body)?);
                            }
                            "depends" => {
                                // old format
                            }
                            "files" => {
                                files_entries.insert(pkgname.to_owned(), parse_files(&body)?);
                            }
                            _ => {
                                return Err(format_err!("Unknown pathname: {}", pathname));
                            }
                        }
                    } else {
                        return Err(format_err!("Invalid pathname entry: {}", pathname));
                    }
                }
                tar::EntryType::Directory => {}
                _ => {
                    return Err(format_err!("Unknown file type: {}", pathname));
                }
            }
        }

        for (_, desc) in desc_entries {
            let files = files_entries.remove(&desc.name).unwrap_or_default();
            self.entries
                .insert(desc.name.to_owned(), PackageEntry { desc, files });
        }
        Ok(())
    }

    pub fn add(&mut self, package: &super::package::Package) {
        let desc = Desc {
            groups: package.groups().to_owned(),
            license: package.license().to_owned(),
            replaces: package.replaces().to_owned(),
            filename: package.filename().to_os_string(),
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
            backups: package.backups().to_owned(),
            depends: package.depends().to_owned(),
            makedepends: package.makedepends().to_owned(),
            checkdepends: package.checkdepends().to_owned(),
            optdepends: package.optdepends().to_owned(),
        };
        self.entries.insert(
            desc.name.to_owned(),
            PackageEntry {
                desc,
                files: package.files().to_owned(),
            },
        );
    }

    pub fn remove(&mut self, package_name: &str) {
        self.entries.remove(package_name);
    }

    pub fn save(&self, include_files: bool) -> Result<(), failure::Error> {
        let mut tmp_path = self.path.clone().into_os_string();
        tmp_path.push(".progress");
        let file = std::fs::File::create(&tmp_path)?;
        let gz_writer = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(gz_writer);
        for package_entry in self.entries.values() {
            let pathbuf = std::path::PathBuf::from(format!(
                "{}-{}/",
                package_entry.desc.name, package_entry.desc.version
            ));
            {
                let mut dir_header = tar::Header::new_gnu();
                dir_header.set_entry_type(tar::EntryType::Directory);
                dir_header.set_path(&pathbuf)?;
                dir_header.set_mode(0o755);
                dir_header.set_size(0);
                dir_header.set_cksum();
                builder.append(&dir_header, std::io::empty())?;
            }
            {
                let mut desc_header = tar::Header::new_gnu();
                desc_header.set_entry_type(tar::EntryType::Regular);
                desc_header.set_path(pathbuf.join("desc"))?;
                desc_header.set_mode(0o644);
                let desc_vec = into_desc_file(package_entry);
                let desc_bytes = desc_vec.as_slice();
                desc_header.set_size(desc_bytes.len() as u64);
                desc_header.set_cksum();
                builder.append(&desc_header, desc_bytes)?;
            }
            if include_files {
                let mut files_header = tar::Header::new_gnu();
                files_header.set_entry_type(tar::EntryType::Regular);
                files_header.set_path(pathbuf.join("files"))?;
                files_header.set_mode(0o644);
                let files_vec = into_files_file(&package_entry.files);
                let files_bytes = files_vec.as_slice();
                files_header.set_size(files_bytes.len() as u64);
                files_header.set_cksum();
                builder.append(&files_header, files_bytes)?;
            }
        }
        let gz_writer = builder.into_inner()?;
        gz_writer.finish()?;

        if let Some(signer) = self.signer {
            let mut sig_path = self.path.clone().into_os_string();
            sig_path.push(".sig");
            signer.sign(&tmp_path, sig_path)?;
        }

        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}

fn parse_desc(body: &str) -> Result<Desc, failure::Error> {
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
                desc.filename = std::ffi::OsString::from(val);
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
                desc.csize = val.parse()?;
            }
            "ISIZE" => {
                desc.isize = val.parse()?;
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
                desc.builddate = val.parse()?;
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
                return Err(format_err!("Unknown desc entry: {}", key));
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
        None
    }
}

fn each_entry(body: &str) -> EachEntry {
    EachEntry {
        key: "",
        lines: body.lines(),
    }
}

fn into_desc_file(package_entry: &PackageEntry) -> Vec<u8> {
    let mut buf = vec![];
    let desc = &package_entry.desc;
    desc_write_array(&mut buf, b"GROUPS", &desc.groups);
    desc_write_array(&mut buf, b"REPLACES", &desc.replaces);
    desc_write_os_str(&mut buf, b"FILENAME", &desc.filename);
    desc_write_string(&mut buf, b"NAME", &desc.name);
    desc_write_string(&mut buf, b"BASE", &desc.base);
    desc_write_string(&mut buf, b"VERSION", &desc.version);
    desc_write_string(&mut buf, b"DESC", &desc.desc);
    desc_write_u64(&mut buf, b"CSIZE", desc.csize);
    desc_write_u64(&mut buf, b"ISIZE", desc.isize);
    desc_write_string(&mut buf, b"MD5SUM", &desc.md5sum);
    desc_write_string(&mut buf, b"SHA256SUM", &desc.sha256sum);
    desc_write_string(&mut buf, b"PGPSIG", &desc.pgpsig);
    desc_write_string(&mut buf, b"URL", &desc.url);
    desc_write_array(&mut buf, b"LICENSE", &desc.license);
    desc_write_string(&mut buf, b"ARCH", &desc.arch);
    desc_write_u64(&mut buf, b"BUILDDATE", desc.builddate);
    desc_write_string(&mut buf, b"PACKAGER", &desc.packager);
    desc_write_array(&mut buf, b"CONFLICTS", &desc.conflicts);
    desc_write_array(&mut buf, b"PROVIDES", &desc.provides);
    desc_write_array(&mut buf, b"DEPENDS", &desc.depends);
    desc_write_array(&mut buf, b"MAKEDEPENDS", &desc.makedepends);
    desc_write_array(&mut buf, b"CHECKDEPENDS", &desc.checkdepends);
    desc_write_array(&mut buf, b"OPTDEPENDS", &desc.optdepends);
    buf
}

fn desc_write_array(buf: &mut Vec<u8>, key: &[u8], xs: &[String]) {
    if !xs.is_empty() {
        buf.extend_from_slice(b"%");
        buf.extend_from_slice(key);
        buf.extend_from_slice(b"%\n");
        for x in xs {
            buf.extend_from_slice(x.as_bytes());
            buf.extend_from_slice(b"\n");
        }
        buf.extend_from_slice(b"\n");
    }
}

fn desc_write_string(buf: &mut Vec<u8>, key: &[u8], val: &str) {
    if !val.is_empty() {
        buf.extend_from_slice(b"%");
        buf.extend_from_slice(key);
        buf.extend_from_slice(b"%\n");
        buf.extend_from_slice(val.as_bytes());
        buf.extend_from_slice(b"\n\n");
    }
}

fn desc_write_os_str(buf: &mut Vec<u8>, key: &[u8], val: &std::ffi::OsStr) {
    if !val.is_empty() {
        use std::os::unix::ffi::OsStrExt;

        buf.extend_from_slice(b"%");
        buf.extend_from_slice(key);
        buf.extend_from_slice(b"%\n");
        buf.extend(val.as_bytes());
        buf.extend_from_slice(b"\n\n");
    }
}

fn desc_write_u64(buf: &mut Vec<u8>, key: &[u8], val: u64) {
    if val != 0 {
        buf.extend_from_slice(b"%");
        buf.extend_from_slice(key);
        buf.extend_from_slice(b"%\n");
        buf.extend_from_slice(format!("{}", val).as_bytes());
        buf.extend_from_slice(b"\n\n");
    }
}

fn parse_files(body: &str) -> Result<Vec<std::path::PathBuf>, failure::Error> {
    let mut iter = body.lines();

    if let Some("%FILES%") = iter.next() {
        let mut files = vec![];
        for line in iter {
            files.push(std::path::PathBuf::from(line));
        }
        Ok(files)
    } else {
        Err(format_err!("Empty files file"))
    }
}

fn into_files_file(files: &[std::path::PathBuf]) -> Vec<u8> {
    let mut buf = vec![];
    buf.extend_from_slice(b"%FILES%\n");
    for file in files {
        use std::os::unix::ffi::OsStrExt;

        buf.extend(file.as_os_str().as_bytes());
        buf.extend_from_slice(b"\n");
    }
    buf
}
