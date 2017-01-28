extern crate serde;
extern crate serde_yaml;
extern crate std;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub name: String,
    pub package_key: Option<String>,
    pub repo_key: Option<String>,
    pub srcdest: String,
    pub logdest: String,
    pub pkgbuild: String,
    pub builds: std::collections::HashMap<super::builder::Arch, BuildConfig>,
}

#[derive(Debug, Deserialize)]
pub struct BuildConfig {
    pub chroot: String,
}

impl Config {
    pub fn from_reader<R>(reader: R) -> serde_yaml::Result<Self>
        where R: std::io::Read
    {
        serde_yaml::from_reader(reader)
    }

    pub fn repo_dir(&self, arch: &super::builder::Arch) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.name).join("os").join(format!("{}", arch))
    }

    pub fn db_path(&self, arch: &super::builder::Arch) -> std::path::PathBuf {
        let mut path = self.repo_dir(arch).join(&self.name).into_os_string();
        path.push(".db");
        return std::path::PathBuf::from(path);
    }

    pub fn files_path(&self, arch: &super::builder::Arch) -> std::path::PathBuf {
        let mut path = self.repo_dir(arch).join(&self.name).into_os_string();
        path.push(".files");
        return std::path::PathBuf::from(path);
    }

    pub fn abs_path(&self, arch: &super::builder::Arch) -> std::path::PathBuf {
        let mut path = self.repo_dir(arch).join(&self.name).into_os_string();
        path.push(".abs.tar.gz");
        return std::path::PathBuf::from(path);
    }

    pub fn package_dir(&self, package_name: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.pkgbuild).join(package_name)
    }
}
