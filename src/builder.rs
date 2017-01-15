extern crate gpgme;
extern crate std;
extern crate tempdir;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Gpgme(gpgme::Error),
    Custom(&'static str),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<super::signer::Error> for Error {
    fn from(e: super::signer::Error) -> Self {
        match e {
            super::signer::Error::Io(e) => Error::Io(e),
            super::signer::Error::Gpgme(e) => Error::Gpgme(e),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(non_camel_case_types)]
pub enum Arch {
    I686,
    X86_64,
}

#[derive(Debug, Clone)]
pub struct ChrootHelper<'a> {
    chroot_dir: &'a str,
    arch: Arch,
}

impl<'a> ChrootHelper<'a> {
    pub fn new(chroot_dir: &'a str, arch: Arch) -> Self {
        ChrootHelper {
            chroot_dir: chroot_dir,
            arch: arch,
        }
    }

    pub fn makechrootpkg<P: AsRef<std::path::Path>,
                         Q: AsRef<std::path::Path>,
                         R: AsRef<std::path::Path>,
                         S: AsRef<std::path::Path>>
        (&self,
         package_dir: P,
         srcdest: Q,
         pkgdest: R,
         logdest: S)
         -> Result<(), Error> {
        let current_dir_buf = try!(std::env::current_dir());
        let current_dir = current_dir_buf.as_path();
        let mut srcdest_arg = std::ffi::OsString::from("SRCDEST=");
        srcdest_arg.push(current_dir.join(srcdest));
        let mut pkgdest_arg = std::ffi::OsString::from("PKGDEST=");
        pkgdest_arg.push(current_dir.join(pkgdest));
        let mut logdest_arg = std::ffi::OsString::from("LOGDEST=");
        logdest_arg.push(current_dir.join(logdest));

        let mut cmd = std::process::Command::new("sudo");
        cmd.current_dir(package_dir)
            .arg("env")
            .arg(srcdest_arg)
            .arg(pkgdest_arg)
            .arg(logdest_arg)
            .arg("makechrootpkg")
            .arg("-cur")
            .arg(self.chroot_dir);
        info!("{:?}", cmd);
        let status = try!(cmd.status());
        if status.success() {
            Ok(())
        } else {
            Err(Error::Custom("makechrootpkg failed"))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Builder<'a> {
    signer: Option<&'a super::signer::Signer<'a>>,
    srcdest: &'a str,
    logdest: &'a str,
}

impl<'a> Builder<'a> {
    pub fn new(signer: Option<&'a super::signer::Signer<'a>>,
               srcdest: &'a str,
               logdest: &'a str)
               -> Self {
        Builder {
            signer: signer,
            srcdest: srcdest,
            logdest: logdest,
        }
    }

    pub fn build_package<P: AsRef<std::path::Path>>(&self,
                                                    package_dir: &str,
                                                    repo_dir: P,
                                                    chroot_helper: &ChrootHelper)
                                                    -> Result<Vec<std::path::PathBuf>, Error> {
        let tempdir = try!(tempdir::TempDir::new("guzuta-pkgdest"));
        try!(chroot_helper.makechrootpkg(package_dir, self.srcdest, &tempdir, self.logdest));
        let mut paths = vec![];
        for entry in try!(std::fs::read_dir(tempdir)) {
            let entry = try!(entry);
            let dest = repo_dir.as_ref().join(entry.file_name());
            if dest.read_link().is_ok() {
                // Unlink symlink created by makechrootpkg
                try!(std::fs::remove_file(&dest));
            }
            try!(std::fs::copy(entry.path(), &dest));
            if let Some(ref signer) = self.signer {
                let mut sig_dest = dest.clone().into_os_string();
                sig_dest.push(".sig");
                try!(signer.sign(&dest, sig_dest));
            }
            paths.push(dest);
        }
        Ok(paths)
    }
}
