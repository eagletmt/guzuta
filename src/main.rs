use anyhow::{Context, Result};

/// Custom repository manager for ArchLinux pacman
#[derive(Debug, clap::Parser)]
#[command(version, about)]
struct Args {
    #[command(subcommand)]
    command: Subcommand,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Build package in systemd-nspawn environment
    Build(BuildArgs),
    /// Add PACKAGE_PATH to DB_PATH
    RepoAdd(RepoAddArgs),
    /// Remove PACKAGE_NAME from DB_PATH
    RepoRemove(RepoRemoveArgs),
    /// Add PACKAGE_PATH to FILES_PATH
    FilesAdd(FilesAddArgs),
    /// Remove PACKAGE_NAME from FILES_PATH
    FilesRemove(FilesRemoveArgs),
    /// Manage repository with S3
    Omakase(OmakaseArgs),
}

#[derive(Debug, clap::Args)]
struct BuildArgs {
    /// Path to chroot top
    #[arg(long)]
    chroot_dir: std::path::PathBuf,
    /// GPG key to sign packages
    #[arg(long)]
    package_key: Option<String>,
    /// Path to the directory to store sources
    #[arg(long)]
    srcdest: Option<std::path::PathBuf>,
    /// Path to the directory to store logs
    #[arg(long)]
    logdest: Option<std::path::PathBuf>,
    /// Path to the repository directory
    #[arg(long)]
    repo_dir: std::path::PathBuf,
    /// GPG key to sign repository database
    #[arg(long)]
    repo_key: Option<String>,
    /// Architecture
    #[arg(long)]
    arch: guzuta::Arch,
    /// Repository name
    #[arg(long)]
    repo_name: String,
    ///Path to the directory containing PKGBUILD
    package_dir: std::path::PathBuf,
}

#[derive(Debug, clap::Args)]
struct RepoAddArgs {
    /// GPG key to sign repository database
    #[arg(long)]
    repo_key: Option<String>,
    /// Path to package to be added
    package_path: std::path::PathBuf,
    /// Path to repository database
    db_path: std::path::PathBuf,
}

#[derive(Debug, clap::Args)]
struct RepoRemoveArgs {
    /// GPG key to sign repository database
    #[arg(long)]
    repo_key: Option<String>,
    /// Package name to be removed
    package_name: String,
    /// Path to repository database
    db_path: std::path::PathBuf,
}

#[derive(Debug, clap::Args)]
struct FilesAddArgs {
    /// GPG key to sign repository database
    #[arg(long)]
    repo_key: Option<String>,
    /// Path to package to be added
    package_path: std::path::PathBuf,
    /// Path to repository database
    files_path: std::path::PathBuf,
}

#[derive(Debug, clap::Args)]
struct FilesRemoveArgs {
    /// GPG key to sign repository database
    #[arg(long)]
    repo_key: Option<String>,
    /// Package name to be removed
    package_name: String,
    /// Path to repository database
    files_path: std::path::PathBuf,
}

#[derive(Debug, clap::Args)]
struct OmakaseArgs {
    #[command(subcommand)]
    command: OmakaseCommand,
}

#[derive(Debug, clap::Subcommand)]
enum OmakaseCommand {
    /// Build PACKAGE_NAME
    Build(OmakaseBuildArgs),
    /// Remove PACKAGE_NAME
    Remove(OmakaseRemoveArgs),
}

#[derive(Debug, clap::Args)]
struct OmakaseBuildArgs {
    package_name: String,
}

#[derive(Debug, clap::Args)]
struct OmakaseRemoveArgs {
    package_name: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    use clap::Parser as _;
    let args = Args::parse();

    run_subcommand(args.command).await
}

async fn run_subcommand(subcommand: Subcommand) -> Result<()> {
    match subcommand {
        Subcommand::Build(args) => build(args).await?,
        Subcommand::RepoAdd(args) => {
            repo_add(args).await;
        }
        Subcommand::RepoRemove(args) => {
            repo_remove(args).await;
        }
        Subcommand::FilesAdd(args) => {
            files_add(args).await;
        }
        Subcommand::FilesRemove(args) => {
            files_remove(args).await;
        }
        Subcommand::Omakase(omakase_args) => match omakase_args.command {
            OmakaseCommand::Build(args) => {
                omakase_build(args).await;
            }
            OmakaseCommand::Remove(args) => {
                omakase_remove(args).await;
            }
        },
    };

    Ok(())
}

async fn build(args: BuildArgs) -> Result<()> {
    let chroot = guzuta::ChrootHelper::new(&args.chroot_dir, args.arch);
    let package_signer = args
        .package_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let srcdest = args
        .srcdest
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    let logdest = args
        .logdest
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("."));
    let builder = guzuta::Builder::new(package_signer.as_ref(), srcdest, logdest);

    let repo_signer = args
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let repo_signer = repo_signer.as_ref();
    let mut db_path = args.repo_dir.join(&args.repo_name).into_os_string();
    db_path.push(".db");
    let mut files_path = args.repo_dir.join(&args.repo_name).into_os_string();
    files_path.push(".files");
    let mut db_repo = guzuta::Repository::new(std::path::PathBuf::from(db_path), repo_signer);
    let mut files_repo = guzuta::Repository::new(std::path::PathBuf::from(files_path), repo_signer);
    db_repo.load().with_context(|| {
        format!(
            "Unable to load database repository from {}",
            db_repo.path().display()
        )
    })?;
    files_repo.load().with_context(|| {
        format!(
            "Unable to load files repository from {}",
            files_repo.path().display()
        )
    })?;

    let package_dir = &args.package_dir;
    let package_paths = builder
        .build_package(package_dir, args.repo_dir, &chroot)
        .await
        .with_context(|| format!("Unable to build package in {}", package_dir.display()))?;

    for path in package_paths {
        let package = guzuta::Package::load(&path)
            .unwrap_or_else(|_| panic!("Unable to load built package at {}", path.display()));
        db_repo.add(&package);
        files_repo.add(&package);
    }

    db_repo.save(false).await.unwrap_or_else(|_| {
        panic!(
            "Unable to save database repository to {}",
            db_repo.path().display()
        )
    });
    files_repo.save(true).await.unwrap_or_else(|_| {
        panic!(
            "Unable to save files repository to {}",
            files_repo.path().display()
        )
    });

    Ok(())
}

async fn repo_add(args: RepoAddArgs) {
    let signer = args
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let package_path = args.package_path;
    let package = guzuta::Package::load(&package_path)
        .unwrap_or_else(|_| panic!("Unable to load package {}", package_path.display()));
    let mut repository = guzuta::Repository::new(args.db_path, signer.as_ref());

    repository.load().unwrap_or_else(|_| {
        panic!(
            "Unable to load database repository from {}",
            repository.path().display()
        )
    });
    repository.add(&package);
    repository.save(false).await.unwrap_or_else(|_| {
        panic!(
            "Unable to save database repository to {}",
            repository.path().display()
        )
    });
}

async fn repo_remove(args: RepoRemoveArgs) {
    let signer = args
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let mut repository = guzuta::Repository::new(args.db_path, signer.as_ref());

    repository.load().unwrap_or_else(|_| {
        panic!(
            "Unable to load database repository from {}",
            repository.path().display()
        )
    });
    repository.remove(&args.package_name);
    repository.save(false).await.unwrap_or_else(|_| {
        panic!(
            "Unable to save database repository to {}",
            repository.path().display()
        )
    });
}

async fn files_add(args: FilesAddArgs) {
    let signer = args
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let package_path = args.package_path;
    let package = guzuta::Package::load(&package_path)
        .unwrap_or_else(|_| panic!("Unable to load package {}", package_path.display()));
    let mut repository = guzuta::Repository::new(args.files_path, signer.as_ref());

    repository.load().unwrap_or_else(|_| {
        panic!(
            "Unable to load files repository from {}",
            repository.path().display()
        )
    });
    repository.add(&package);
    repository.save(true).await.unwrap_or_else(|_| {
        panic!(
            "Unable to save files repository to {}",
            repository.path().display()
        )
    });
}

async fn files_remove(args: FilesRemoveArgs) {
    let signer = args
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let mut repository = guzuta::Repository::new(args.files_path, signer.as_ref());

    repository.load().unwrap_or_else(|_| {
        panic!(
            "Unable to load files repository from {}",
            repository.path().display()
        )
    });
    repository.remove(&args.package_name);
    repository.save(true).await.unwrap_or_else(|_| {
        panic!(
            "Unable to save files repository to {}",
            repository.path().display()
        )
    });
}

async fn omakase_build(args: OmakaseBuildArgs) {
    let file = std::fs::File::open(".guzuta.yml").expect("Unable to open .guzuta.yml");
    let config =
        guzuta::omakase::Config::from_reader(file).expect("Unable to load YAML from .guzuta.yml");
    let package_signer = config
        .package_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let repo_signer = config
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let repo_signer = repo_signer.as_ref();
    let builder = guzuta::Builder::new(package_signer.as_ref(), &config.srcdest, &config.logdest);
    let s3 = if let Some(ref s3_config) = config.s3 {
        Some(guzuta::omakase::S3::new(s3_config.clone()).await)
    } else {
        None
    };

    for (&arch, build_config) in &config.builds {
        let chroot = guzuta::ChrootHelper::new(&build_config.chroot, arch);
        let repo_dir = config.repo_dir(arch);
        let package_dir = config.package_dir(&args.package_name);

        std::fs::create_dir_all(repo_dir.as_path()).unwrap_or_else(|_| {
            panic!(
                "Unable to create directories {}",
                repo_dir.as_path().display()
            )
        });

        if let Some(ref s3) = s3 {
            s3.download_repository(&config, arch)
                .await
                .expect("Unable to download files from S3");
        }

        let mut db_repo = guzuta::Repository::new(config.db_path(arch), repo_signer);
        let mut files_repo = guzuta::Repository::new(config.files_path(arch), repo_signer);
        db_repo.load().unwrap_or_else(|_| {
            panic!(
                "Unable to load database repository from {}",
                db_repo.path().display()
            )
        });
        files_repo.load().unwrap_or_else(|_| {
            panic!(
                "Unable to load files repository from {}",
                files_repo.path().display()
            )
        });

        let package_paths = builder
            .build_package(package_dir.as_path(), repo_dir, &chroot)
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Unable to build package in {}",
                    package_dir.as_path().display()
                )
            });
        for path in &package_paths {
            let package = guzuta::Package::load(path)
                .unwrap_or_else(|_| panic!("Unable to load package {}", path.display()));
            db_repo.add(&package);
            files_repo.add(&package);
        }

        db_repo.save(false).await.unwrap_or_else(|_| {
            panic!(
                "Unable to save database repository to {}",
                db_repo.path().display()
            )
        });
        files_repo.save(true).await.unwrap_or_else(|_| {
            panic!(
                "Unable to save files repository to {}",
                files_repo.path().display()
            )
        });

        if let Some(ref s3) = s3 {
            s3.upload_repository(&config, arch, &package_paths)
                .await
                .expect("Unable to upload files to S3");
        }
    }
}

async fn omakase_remove(args: OmakaseRemoveArgs) {
    let file = std::fs::File::open(".guzuta.yml").expect("Unable to open .guzuta.yml");
    let config =
        guzuta::omakase::Config::from_reader(file).expect("Unable to load YAML from .guzuta.yml");
    let repo_signer = config
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key.as_str()));
    let repo_signer = repo_signer.as_ref();
    let s3 = if let Some(ref s3_config) = config.s3 {
        Some(guzuta::omakase::S3::new(s3_config.clone()).await)
    } else {
        None
    };

    for &arch in config.builds.keys() {
        if let Some(ref s3) = s3 {
            s3.download_repository(&config, arch)
                .await
                .expect("Unable to download files from S3");
        }

        let mut db_repo = guzuta::Repository::new(config.db_path(arch), repo_signer);
        let mut files_repo = guzuta::Repository::new(config.files_path(arch), repo_signer);
        db_repo.load().unwrap_or_else(|_| {
            panic!(
                "Unable to load database repository from {}",
                db_repo.path().display()
            )
        });
        files_repo.load().unwrap_or_else(|_| {
            panic!(
                "Unable to load files repository from {}",
                files_repo.path().display()
            )
        });

        db_repo.remove(&args.package_name);
        files_repo.remove(&args.package_name);
        db_repo.save(false).await.unwrap_or_else(|_| {
            panic!(
                "Unable to save database repository to {}",
                db_repo.path().display()
            )
        });
        files_repo.save(true).await.unwrap_or_else(|_| {
            panic!(
                "Unable to save files repository to {}",
                files_repo.path().display()
            )
        });

        if let Some(ref s3) = s3 {
            let paths: Vec<&str> = vec![];
            s3.upload_repository(&config, arch, &paths)
                .await
                .expect("Unable to upload files to S3");
        }
    }
}
