extern crate flate2;
extern crate std;
extern crate tar;
extern crate tempdir;

pub struct Abs<'a> {
    repo_name: &'a str,
    abs_path: std::path::PathBuf,
}

impl<'a> Abs<'a> {
    pub fn new<P: AsRef<std::path::Path>>(repo_name: &'a str, abs_path: P) -> Self {
        Abs {
            repo_name: repo_name,
            abs_path: abs_path.as_ref().to_path_buf(),
        }
    }

    pub fn add<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(&self,
                                                                     package_dir: P,
                                                                     srcdest: Q) {
        let root = tempdir::TempDir::new("guzuta-abs-root").unwrap();
        self.unarchive(root.as_ref(), self.abs_path.as_path());
        self.add_srcpkg(root.as_ref(), package_dir.as_ref(), srcdest);
        self.archive(root, self.abs_path.as_path());
    }

    fn unarchive<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(&self,
                                                                       root_dir: P,
                                                                       abs_path: Q) {
        match std::fs::File::open(abs_path) {
            Ok(file) => self.unarchive_file(root_dir, file),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    panic!("{:?}", e);
                }
            }
        }
    }

    fn unarchive_file<P: AsRef<std::path::Path>, R: std::io::Read>(&self,
                                                                   root_dir: P,
                                                                   abs_file: R) {
        let gz_reader = flate2::read::GzDecoder::new(abs_file).unwrap();
        let mut tar_reader = tar::Archive::new(gz_reader);
        tar_reader.unpack(root_dir).unwrap();
    }

    fn add_srcpkg<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>, R: AsRef<std::path::Path>>
        (&self,
         root_dir: P,
         package_dir: Q,
         srcdest: R) {
        let current_dir_buf = std::env::current_dir().expect("Unable to get current directory");
        let current_dir = current_dir_buf.as_path();
        let srcdest = current_dir.join(srcdest);
        let srcpkgdest = tempdir::TempDir::new("guzuta-abs-srcpkgdest").unwrap();
        let builddir = tempdir::TempDir::new("guzuta-abs-builddir").unwrap();
        let mut cmd = std::process::Command::new("makepkg");
        cmd.env("SRCDEST", srcdest)
            .env("SRCPKGDEST", srcpkgdest.path())
            .env("BUILDDIR", builddir.path())
            .current_dir(package_dir.as_ref())
            .arg("--source");
        info!("{:?}", cmd);
        let status = cmd.status().unwrap();
        if !status.success() {
            panic!("makepkg --source failed");
        }

        for entry in std::fs::read_dir(srcpkgdest.path()).unwrap() {
            let path = entry.unwrap().path();
            info!("Unarchive source package {} into {}",
                  path.display(),
                  root_dir.as_ref().display());
            self.unarchive(root_dir.as_ref().join(self.repo_name), path);
            return;
        }
        panic!("No source pakcage is generated");
    }

    fn archive<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>(&self,
                                                                     root_dir: P,
                                                                     abs_path: Q) {
        let file = std::fs::File::create(abs_path.as_ref()).unwrap();
        let gz_writer = flate2::write::GzEncoder::new(file, flate2::Compression::Default);
        let mut builder = tar::Builder::new(gz_writer);
        self.archive_path(&mut builder, root_dir.as_ref(), root_dir.as_ref());
        let gz_writer = builder.into_inner().unwrap();
        gz_writer.finish().unwrap();
    }

    fn archive_path<W: std::io::Write,
                    P: AsRef<std::path::Path>,
                    Q: AsRef<std::path::Path>>
        (&self, mut builder: &mut tar::Builder<W>, root_dir: P, path: Q) {
        let path_in_archive = path.as_ref().strip_prefix(root_dir.as_ref()).unwrap();
        if path.as_ref().is_dir() {
            if !path_in_archive.as_os_str().is_empty() {
                let mut path_in_archive = path_in_archive.to_path_buf().into_os_string();
                path_in_archive.push("/");
                builder.append_dir(path_in_archive, path.as_ref()).unwrap();
            }
            for entry in std::fs::read_dir(path.as_ref()).unwrap() {
                self.archive_path(&mut builder, root_dir.as_ref(), entry.unwrap().path());
            }
        } else if path.as_ref().is_file() {
            let mut file = std::fs::File::open(path.as_ref()).unwrap();
            builder.append_file(path_in_archive, &mut file).unwrap();
        } else {
            panic!("Invalid file type: {}", path.as_ref().display());
        }
    }
}
