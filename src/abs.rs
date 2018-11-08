extern crate failure;
extern crate flate2;
extern crate std;
extern crate tar;
extern crate tempdir;

#[derive(Debug, Clone)]
pub struct Abs<'a> {
    repo_name: &'a str,
    abs_path: std::path::PathBuf,
}

impl<'a> Abs<'a> {
    pub fn new<P>(repo_name: &'a str, abs_path: P) -> Self
    where
        P: AsRef<std::path::Path>,
    {
        Abs {
            repo_name,
            abs_path: abs_path.as_ref().to_path_buf(),
        }
    }

    pub fn path(&self) -> &std::path::Path {
        self.abs_path.as_path()
    }

    pub fn add<P, Q>(&self, package_dir: P, srcdest: Q) -> Result<(), failure::Error>
    where
        P: AsRef<std::path::Path>,
        Q: AsRef<std::path::Path>,
    {
        let root = tempdir::TempDir::new("guzuta-abs-root")?;
        let root = root.as_ref();
        self.unarchive(root, self.abs_path.as_path())?;
        self.add_srcpkg(root, package_dir, srcdest)?;
        self.archive(root, self.abs_path.as_path())?;
        Ok(())
    }

    pub fn remove(&self, package_name: &str) -> Result<(), failure::Error> {
        let root = tempdir::TempDir::new("guzuta-abs-root")?;
        self.unarchive(root.as_ref(), self.abs_path.as_path())?;
        std::fs::remove_dir_all(root.path().join(self.repo_name).join(package_name))?;
        self.archive(root, self.abs_path.as_path())?;
        Ok(())
    }

    fn unarchive<P, Q>(&self, root_dir: P, abs_path: Q) -> Result<(), std::io::Error>
    where
        P: AsRef<std::path::Path>,
        Q: AsRef<std::path::Path>,
    {
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

    fn unarchive_file<P, R>(&self, root_dir: P, abs_file: R) -> Result<(), std::io::Error>
    where
        P: AsRef<std::path::Path>,
        R: std::io::Read,
    {
        let gz_reader = flate2::read::GzDecoder::new(abs_file);
        let mut tar_reader = tar::Archive::new(gz_reader);
        tar_reader.unpack(root_dir)?;
        Ok(())
    }

    fn add_srcpkg<P, Q, R>(
        &self,
        root_dir: P,
        package_dir: Q,
        srcdest: R,
    ) -> Result<(), failure::Error>
    where
        P: AsRef<std::path::Path>,
        Q: AsRef<std::path::Path>,
        R: AsRef<std::path::Path>,
    {
        let package_dir = package_dir.as_ref();
        let root_dir = root_dir.as_ref();
        let current_dir_buf = std::env::current_dir()?;
        let current_dir = current_dir_buf.as_path();
        let srcdest = current_dir.join(srcdest);
        let srcpkgdest = tempdir::TempDir::new("guzuta-abs-srcpkgdest")?;
        let builddir = tempdir::TempDir::new("guzuta-abs-builddir")?;
        let mut cmd = std::process::Command::new("makepkg");
        cmd.env("SRCDEST", srcdest)
            .env("SRCPKGDEST", srcpkgdest.path())
            .env("BUILDDIR", builddir.path())
            .current_dir(package_dir)
            .arg("--source");
        info!("{:?}", cmd);
        let status = cmd.status()?;
        if !status.success() {
            return Err(format_err!("makepkg --source failed"));
        }

        if let Some(entry) = std::fs::read_dir(srcpkgdest.path())?.next() {
            let entry = entry?;
            let symlink_source_package_path = package_dir.join(entry.file_name());
            if symlink_source_package_path.read_link().is_ok() {
                info!("Unlink symlink {}", symlink_source_package_path.display());
                std::fs::remove_file(symlink_source_package_path)?;
            }
            let path = entry.path();
            info!(
                "Unarchive source package {} into {}",
                path.display(),
                root_dir.display()
            );
            self.unarchive(root_dir.join(self.repo_name), path)?;
            Ok(())
        } else {
            Err(format_err!("No source pakcage is generated"))
        }
    }

    fn archive<P, Q>(&self, root_dir: P, abs_path: Q) -> Result<(), std::io::Error>
    where
        P: AsRef<std::path::Path>,
        Q: AsRef<std::path::Path>,
    {
        let root_dir = root_dir.as_ref();
        let file = std::fs::File::create(abs_path.as_ref())?;
        let gz_writer = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(gz_writer);
        self.archive_path(&mut builder, root_dir, root_dir)?;
        let gz_writer = builder.into_inner()?;
        gz_writer.finish()?;
        Ok(())
    }

    fn archive_path<W, P, Q>(
        &self,
        mut builder: &mut tar::Builder<W>,
        root_dir: P,
        path: Q,
    ) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
        P: AsRef<std::path::Path>,
        Q: AsRef<std::path::Path>,
    {
        let root_dir = root_dir.as_ref();
        let path = path.as_ref();
        let path_in_archive = path.strip_prefix(root_dir).expect("Failed to strip prefix");
        if path.is_dir() {
            if !path_in_archive.as_os_str().is_empty() {
                let mut path_in_archive = path_in_archive.to_path_buf().into_os_string();
                path_in_archive.push("/");
                builder.append_dir(path_in_archive, path)?;
            }
            for entry in std::fs::read_dir(path)? {
                self.archive_path(&mut builder, root_dir, entry?.path())?;
            }
            Ok(())
        } else if path.is_file() {
            let mut file = std::fs::File::open(path)?;
            builder.append_file(path_in_archive, &mut file)?;
            Ok(())
        } else {
            // Ignore unknown file type
            Ok(())
        }
    }
}
