extern crate std;
extern crate tempdir;

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
         logdest: S) {
        let current_dir_buf = std::env::current_dir().expect("Unable to get current directory");
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
        let status = cmd.status().expect("makechrootpkg failed to start");
        if !status.success() {
            panic!("makechrootpkg failed");
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
                                                    -> Vec<std::path::PathBuf> {
        let tempdir = tempdir::TempDir::new("guzuta-pkgdest")
            .expect("Unable to create temporary directory");
        chroot_helper.makechrootpkg(package_dir, self.srcdest, &tempdir, self.logdest);
        let mut paths = vec![];
        for entry in std::fs::read_dir(&tempdir).expect("read_dir failed") {
            let entry = entry.unwrap();
            let dest = repo_dir.as_ref().join(entry.file_name());
            if dest.read_link().is_ok() {
                // Unlink symlink created by makechrootpkg
                std::fs::remove_file(&dest).expect("Unable to unlink symlinked package");
            }
            std::fs::copy(entry.path(), &dest).expect("Failed to copy package");
            if let Some(ref signer) = self.signer {
                let mut sig_dest = dest.clone().into_os_string();
                sig_dest.push(".sig");
                signer.sign(&dest, sig_dest);
            }
            paths.push(dest);
        }
        return paths;
    }
}
