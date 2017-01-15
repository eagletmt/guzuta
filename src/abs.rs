extern crate flate2;
extern crate std;
extern crate tar;
extern crate tempdir;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Custom(&'static str),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

#[derive(Debug, Clone)]
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
                                                                     srcdest: Q)
                                                                     -> Result<(), Error> {
        let root = try!(tempdir::TempDir::new("guzuta-abs-root"));
        try!(self.unarchive(root.as_ref(), self.abs_path.as_path()));
        try!(self.add_srcpkg(root.as_ref(), package_dir, srcdest));
        try!(self.archive(root, self.abs_path.as_path()));
        Ok(())
    }

    pub fn remove(&self, package_name: &str) -> Result<(), Error> {
        let root = try!(tempdir::TempDir::new("guzuta-abs-root"));
        try!(self.unarchive(root.as_ref(), self.abs_path.as_path()));
        try!(std::fs::remove_dir_all(root.path().join(self.repo_name).join(package_name)));
        try!(self.archive(root, self.abs_path.as_path()));
        Ok(())
    }

    fn unarchive<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>
        (&self,
         root_dir: P,
         abs_path: Q)
         -> Result<(), std::io::Error> {
        match std::fs::File::open(abs_path) {
            Ok(file) => self.unarchive_file(root_dir, file),
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    Err(e)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn unarchive_file<P: AsRef<std::path::Path>, R: std::io::Read>
        (&self,
         root_dir: P,
         abs_file: R)
         -> Result<(), std::io::Error> {
        let gz_reader = try!(flate2::read::GzDecoder::new(abs_file));
        let mut tar_reader = tar::Archive::new(gz_reader);
        try!(tar_reader.unpack(root_dir));
        Ok(())
    }

    fn add_srcpkg<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>, R: AsRef<std::path::Path>>
        (&self,
         root_dir: P,
         package_dir: Q,
         srcdest: R)
         -> Result<(), Error> {
        let current_dir_buf = try!(std::env::current_dir());
        let current_dir = current_dir_buf.as_path();
        let srcdest = current_dir.join(srcdest);
        let srcpkgdest = try!(tempdir::TempDir::new("guzuta-abs-srcpkgdest"));
        let builddir = try!(tempdir::TempDir::new("guzuta-abs-builddir"));
        let mut cmd = std::process::Command::new("makepkg");
        cmd.env("SRCDEST", srcdest)
            .env("SRCPKGDEST", srcpkgdest.path())
            .env("BUILDDIR", builddir.path())
            .current_dir(package_dir.as_ref())
            .arg("--source");
        info!("{:?}", cmd);
        let status = try!(cmd.status());
        if !status.success() {
            return Err(Error::Custom("makepkg --source failed"));
        }

        for entry in try!(std::fs::read_dir(srcpkgdest.path())) {
            let path = try!(entry).path();
            info!("Unarchive source package {} into {}",
                  path.display(),
                  root_dir.as_ref().display());
            try!(self.unarchive(root_dir.as_ref().join(self.repo_name), path));
            return Ok(());
        }
        return Err(Error::Custom("No source pakcage is generated"));
    }

    fn archive<P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>
        (&self,
         root_dir: P,
         abs_path: Q)
         -> Result<(), std::io::Error> {
        let file = try!(std::fs::File::create(abs_path.as_ref()));
        let gz_writer = flate2::write::GzEncoder::new(file, flate2::Compression::Default);
        let mut builder = tar::Builder::new(gz_writer);
        try!(self.archive_path(&mut builder, root_dir.as_ref(), root_dir.as_ref()));
        let gz_writer = try!(builder.into_inner());
        try!(gz_writer.finish());
        Ok(())
    }

    fn archive_path<W: std::io::Write, P: AsRef<std::path::Path>, Q: AsRef<std::path::Path>>
        (&self,
         mut builder: &mut tar::Builder<W>,
         root_dir: P,
         path: Q)
         -> Result<(), std::io::Error> {
        let path_in_archive =
            path.as_ref().strip_prefix(root_dir.as_ref()).expect("Failed to strip prefix");
        if path.as_ref().is_dir() {
            if !path_in_archive.as_os_str().is_empty() {
                let mut path_in_archive = path_in_archive.to_path_buf().into_os_string();
                path_in_archive.push("/");
                try!(builder.append_dir(path_in_archive, path.as_ref()));
            }
            for entry in try!(std::fs::read_dir(path.as_ref())) {
                try!(self.archive_path(&mut builder, root_dir.as_ref(), try!(entry).path()));
            }
            Ok(())
        } else if path.as_ref().is_file() {
            let mut file = try!(std::fs::File::open(path.as_ref()));
            try!(builder.append_file(path_in_archive, &mut file));
            Ok(())
        } else {
            // Ignore unknown file type
            Ok(())
        }
    }
}
