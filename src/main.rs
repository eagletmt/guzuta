#[macro_use]
extern crate clap;
extern crate env_logger;
extern crate guzuta;

fn main() {
    env_logger::init().expect("Unable to init logger");

    let app = clap::App::new("guzuta")
        .version(crate_version!())
        .about("Custom repository manager for ArchLinux pacman")
        .setting(clap::AppSettings::SubcommandRequired)
        .subcommand(clap::SubCommand::with_name("build")
                        .about("Build package in systemd-nspawn environment")
                        .arg(clap::Arg::with_name("chroot-dir")
                                 .long("chroot-dir")
                                 .takes_value(true)
                                 .required(true)
                                 .help("Path to chroot top"))
                        .arg(clap::Arg::with_name("package-key")
                                 .long("package-key")
                                 .takes_value(true)
                                 .help("GPG key to sign packages"))
                        .arg(clap::Arg::with_name("srcdest")
                                 .long("srcdest")
                                 .takes_value(true)
                                 .help("Path to the directory to store sources"))
                        .arg(clap::Arg::with_name("logdest")
                                 .long("logdest")
                                 .takes_value(true)
                                 .help("Path to the directory to store logs"))
                        .arg(clap::Arg::with_name("repo-dir")
                                 .long("repo-dir")
                                 .takes_value(true)
                                 .required(true)
                                 .help("Path to the repository directory"))
                        .arg(clap::Arg::with_name("repo-key")
                                 .long("repo-key")
                                 .takes_value(true)
                                 .help("GPG key to sign repository database"))
                        .arg(clap::Arg::with_name("arch")
                                 .long("arch")
                                 .takes_value(true)
                                 .required(true)
                                 .help("Architecture"))
                        .arg(clap::Arg::with_name("repo-name")
                                 .long("repo-name")
                                 .takes_value(true)
                                 .required(true)
                                 .help("Repository name"))
                        .arg(clap::Arg::with_name("PACKAGE_DIR")
                                 .required(true)
                                 .help("Path to the directory containing PKGBUILD")))
        .subcommand(clap::SubCommand::with_name("repo-add")
                        .about("Add PACKAGE_PATH to DB_PATH")
                        .arg(clap::Arg::with_name("repo-key")
                                 .long("repo-key")
                                 .takes_value(true)
                                 .help("GPG key to sign repository database"))
                        .arg(clap::Arg::with_name("PACKAGE_PATH")
                                 .required(true)
                                 .help("Path to package to be added"))
                        .arg(clap::Arg::with_name("DB_PATH")
                                 .required(true)
                                 .help("Path to repository database")))
        .subcommand(clap::SubCommand::with_name("repo-remove")
                        .about("Remove PACKAGE_NAME from DB_PATH")
                        .arg(clap::Arg::with_name("repo-key")
                                 .long("repo-key")
                                 .takes_value(true)
                                 .help("GPG key to sign repository database"))
                        .arg(clap::Arg::with_name("PACKAGE_NAME")
                                 .required(true)
                                 .help("Path to package to be removed"))
                        .arg(clap::Arg::with_name("DB_PATH")
                                 .required(true)
                                 .help("Path to repository database")))
        .subcommand(clap::SubCommand::with_name("files-add")
                        .about("Add PACKAGE_PATH to FILES_PATH")
                        .arg(clap::Arg::with_name("repo-key")
                                 .long("repo-key")
                                 .takes_value(true)
                                 .help("GPG key to sign repository database"))
                        .arg(clap::Arg::with_name("PACKAGE_PATH")
                                 .required(true)
                                 .help("Path to package to be added"))
                        .arg(clap::Arg::with_name("FILES_PATH")
                                 .required(true)
                                 .help("Path to repository database")))
        .subcommand(clap::SubCommand::with_name("files-remove")
                        .about("Remove PACKAGE_NAME from FILES_PATH")
                        .arg(clap::Arg::with_name("repo-key")
                                 .long("repo-key")
                                 .takes_value(true)
                                 .help("GPG key to sign repository database"))
                        .arg(clap::Arg::with_name("PACKAGE_NAME")
                                 .required(true)
                                 .help("Path to package to be removed"))
                        .arg(clap::Arg::with_name("DB_PATH")
                                 .required(true)
                                 .help("Path to repository database")))
        .subcommand(clap::SubCommand::with_name("abs-add")
                        .about("Add source package to abs tarball")
                        .arg(clap::Arg::with_name("srcdest")
                                 .long("srcdest")
                                 .takes_value(true)
                                 .help("Path to the directory to store sources"))
                        .arg(clap::Arg::with_name("repo-name")
                                 .long("repo-name")
                                 .takes_value(true)
                                 .required(true)
                                 .help("Repository name"))
                        .arg(clap::Arg::with_name("PACKAGE_DIR")
                                 .required(true)
                                 .help("Path to the directory containing PKGBUILD"))
                        .arg(clap::Arg::with_name("ABS_PATH")
                                 .required(true)
                                 .help("Path to abs tarball")))
        .subcommand(clap::SubCommand::with_name("abs-remove")
                        .about("Remove source package from abs tarball")
                        .arg(clap::Arg::with_name("repo-name")
                                 .long("repo-name")
                                 .takes_value(true)
                                 .required(true)
                                 .help("Repository name"))
                        .arg(clap::Arg::with_name("PACKAGE_NAME")
                                 .required(true)
                                 .help("Package name to be removed"))
                        .arg(clap::Arg::with_name("ABS_PATH")
                                 .required(true)
                                 .help("Path to abs tarball")))
        .subcommand(clap::SubCommand::with_name("omakase")
                        .about("Manage repository with S3")
                        .subcommand(clap::SubCommand::with_name("build")
                                        .about("Build PACKAGE_NAME")
                                        .arg(clap::Arg::with_name("PACKAGE_NAME")
                                                 .required(true)))
                        .subcommand(clap::SubCommand::with_name("remove")
                                        .about("Remove PACKAGE_NAME")
                                        .arg(clap::Arg::with_name("PACKAGE_NAME")
                                                 .required(true))));
    let matches = app.get_matches();

    run_subcommand(matches.subcommand());
}

fn run_subcommand(subcommand: (&str, Option<&clap::ArgMatches>)) {
    match subcommand {
        ("build", Some(build_command)) => build(build_command),
        ("repo-add", Some(repo_add_command)) => {
            repo_add(repo_add_command);
        }
        ("repo-remove", Some(repo_remove_command)) => {
            repo_remove(repo_remove_command);
        }
        ("files-add", Some(files_add_command)) => {
            files_add(files_add_command);
        }
        ("files-remove", Some(files_remove_command)) => {
            files_remove(files_remove_command);
        }
        ("abs-add", Some(abs_add_command)) => {
            abs_add(abs_add_command);
        }
        ("abs-remove", Some(abs_remove_command)) => {
            abs_remove(abs_remove_command);
        }
        ("omakase", Some(omakase_command)) => {
            match omakase_command.subcommand() {
                ("build", Some(build_command)) => {
                    omakase_build(build_command);
                }
                ("remove", Some(remove_command)) => {
                    omakase_remove(remove_command);
                }
                _ => {
                    panic!("Unknown subcommand");
                }
            }
        }
        _ => {
            panic!("Unknown subcommand");
        }
    }
}

fn build(args: &clap::ArgMatches) {
    let arch = match args.value_of("arch")
              .expect("Unable to get --arch option") {
        "i686" => guzuta::Arch::I686,
        "x86_64" => guzuta::Arch::X86_64,
        "arm" => guzuta::Arch::ARM,
        "armv6h" => guzuta::Arch::ARMV6H,
        "armv7h" => guzuta::Arch::ARMV7H,
        "aarch64" => guzuta::Arch::AARCH64,
        arch => panic!("Unknown architecture: {}", arch),
    };
    let chroot = guzuta::ChrootHelper::new(args.value_of("chroot-dir")
                                               .expect("Unable to get --chroot-dir option"),
                                           arch);
    let package_signer = args.value_of("package-key")
        .map(|key| guzuta::Signer::new(key));
    let srcdest = args.value_of("srcdest").unwrap_or(".");
    let builder = guzuta::Builder::new(package_signer.as_ref(),
                                       srcdest,
                                       args.value_of("logdest").unwrap_or("."));
    let repo_dir = std::path::Path::new(args.value_of("repo-dir")
                                            .expect("Unable to get --repo-dir option"));
    let repo_name = args.value_of("repo-name")
        .expect("Unable to get --repo-name option");
    let package_dir = args.value_of("PACKAGE_DIR")
        .expect("Unable to get PACKAGE_DIR argument");

    let repo_signer = args.value_of("repo-key")
        .map(|key| guzuta::Signer::new(key));
    let repo_signer = repo_signer.as_ref();
    let mut db_path = repo_dir.join(repo_name).into_os_string();
    db_path.push(".db");
    let mut files_path = repo_dir.join(repo_name).into_os_string();
    files_path.push(".files");
    let mut db_repo = guzuta::Repository::new(std::path::PathBuf::from(db_path), repo_signer);
    let mut files_repo = guzuta::Repository::new(std::path::PathBuf::from(files_path), repo_signer);
    db_repo
        .load()
        .expect(&format!("Unable to load database repository from {}",
                         db_repo.path().display()));
    files_repo
        .load()
        .expect(&format!("Unable to load files repository from {}",
                         files_repo.path().display()));
    let mut abs_path = repo_dir.join(repo_name).into_os_string();
    abs_path.push(".abs.tar.gz");
    let abs = guzuta::Abs::new(repo_name, abs_path);

    let package_paths = builder
        .build_package(package_dir, repo_dir, &chroot)
        .expect(&format!("Unable to build package in {}", package_dir));

    for path in package_paths {
        let package = guzuta::Package::load(&path).expect(&format!("Unable to load built package at {}",
                                                                   path.display()));
        db_repo.add(&package);
        files_repo.add(&package);
    }

    abs.add(package_dir, srcdest)
        .expect(&format!("Unable to add {} to abs tarball {}",
                         package_dir,
                         abs.path().display()));
    db_repo
        .save(false)
        .expect(&format!("Unable to save database repository to {}",
                         db_repo.path().display()));
    files_repo
        .save(true)
        .expect(&format!("Unable to save files repository to {}",
                         files_repo.path().display()));
}

fn repo_add(args: &clap::ArgMatches) {
    let signer = args.value_of("repo-key")
        .map(|key| guzuta::Signer::new(key));
    let package_path = args.value_of("PACKAGE_PATH")
        .expect("Unable to get PACKAGE_PATH argument");
    let package = guzuta::Package::load(&package_path).expect(&format!("Unable to load package {}",
                                                                       package_path));
    let mut repository = guzuta::Repository::new(std::path::PathBuf::from(args.value_of("DB_PATH")
                                                                              .expect("Unable to get DB_PATH argument")),
                                                 signer.as_ref());

    repository
        .load()
        .expect(&format!("Unable to load database repository from {}",
                         repository.path().display()));
    repository.add(&package);
    repository
        .save(false)
        .expect(&format!("Unable to save database repository to {}",
                         repository.path().display()));
}

fn repo_remove(args: &clap::ArgMatches) {
    let signer = args.value_of("repo-key")
        .map(|key| guzuta::Signer::new(key));
    let package_name = args.value_of("PACKAGE_NAME")
        .expect("Unable to get PACKAGE_NAME argument");
    let mut repository = guzuta::Repository::new(std::path::PathBuf::from(args.value_of("DB_PATH")
                                                                              .expect("Unable to get DB_PATH argument")),
                                                 signer.as_ref());

    repository
        .load()
        .expect(&format!("Unable to load database repository from {}",
                         repository.path().display()));
    repository.remove(package_name);
    repository
        .save(false)
        .expect(&format!("Unable to save database repository to {}",
                         repository.path().display()));
}

fn files_add(args: &clap::ArgMatches) {
    let signer = args.value_of("repo-key")
        .map(|key| guzuta::Signer::new(key));
    let package_path = args.value_of("PACKAGE_PATH")
        .expect("Unable to get PACKAGE_PATH argument");
    let package = guzuta::Package::load(&package_path).expect(&format!("Unable to load package {}",
                                                                       package_path));
    let mut repository =
        guzuta::Repository::new(std::path::PathBuf::from(args.value_of("FILES_PATH").expect("Unable to get FILES_PATH argument")),
                                signer.as_ref());

    repository
        .load()
        .expect(&format!("Unable to load files repository from {}",
                         repository.path().display()));
    repository.add(&package);
    repository
        .save(true)
        .expect(&format!("Unable to save files repository to {}",
                         repository.path().display()));
}

fn files_remove(args: &clap::ArgMatches) {
    let signer = args.value_of("repo-key")
        .map(|key| guzuta::Signer::new(key));
    let package_name = args.value_of("PACKAGE_NAME")
        .expect("Unable to get PACKAGE_NAME argument");
    let mut repository =
        guzuta::Repository::new(std::path::PathBuf::from(args.value_of("FILES_PATH").expect("Unable to get FILES_PATH argument")),
                                signer.as_ref());

    repository
        .load()
        .expect(&format!("Unable to load files repository from {}",
                         repository.path().display()));
    repository.remove(package_name);
    repository
        .save(true)
        .expect(&format!("Unable to save files repository to {}",
                         repository.path().display()));
}

fn abs_add(args: &clap::ArgMatches) {
    let srcdest = std::path::PathBuf::from(args.value_of("srcdest").unwrap_or("."));
    let repo_name = args.value_of("repo-name")
        .expect("Unable to get --repo-name option");
    let package_dir = args.value_of("PACKAGE_DIR")
        .expect("Unable to get PACKAGE_DIR argument");
    let abs_path = args.value_of("ABS_PATH")
        .expect("Unable to get ABS_PATH argument");

    let abs = guzuta::Abs::new(repo_name, abs_path);
    abs.add(package_dir, srcdest)
        .expect(&format!("Unable to add {} to abs tarball {}",
                         package_dir,
                         abs.path().display()));
}

fn abs_remove(args: &clap::ArgMatches) {
    let repo_name = args.value_of("repo-name")
        .expect("Unable to get --repo-name option");
    let package_name = args.value_of("PACKAGE_NAME")
        .expect("Unable to get PACKAGE_NAME argument");
    let abs_path = args.value_of("ABS_PATH")
        .expect("Unable to get ABS_PATH argument");

    let abs = guzuta::Abs::new(repo_name, abs_path);
    abs.remove(package_name)
        .expect(&format!("Unable to remove {} from abs tarball {}",
                         package_name,
                         abs.path().display()));
}

fn omakase_build(args: &clap::ArgMatches) {
    let package_name = args.value_of("PACKAGE_NAME")
        .expect("Unable to get PACKAGE_NAME argument");
    let file = std::fs::File::open(".guzuta.yml").expect("Unable to open .guzuta.yml");
    let config =
        guzuta::omakase::Config::from_reader(file).expect("Unable to load YAML from .guzuta.yml");
    let package_signer = config
        .package_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key));
    let repo_signer = config
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key));
    let repo_signer = repo_signer.as_ref();
    let builder = guzuta::Builder::new(package_signer.as_ref(), &config.srcdest, &config.logdest);
    let s3 = config
        .s3
        .as_ref()
        .map(|s3_config| guzuta::omakase::S3::new(s3_config));

    for (arch, build_config) in &config.builds {
        let chroot = guzuta::ChrootHelper::new(&build_config.chroot, arch.clone());
        let repo_dir = config.repo_dir(arch);
        let db_path = config.db_path(arch);
        let files_path = config.files_path(arch);
        let abs_path = config.abs_path(arch);
        let package_dir = config.package_dir(package_name);

        std::fs::create_dir_all(repo_dir.as_path()).expect(&format!("Unable to create directories {}", repo_dir.as_path().display()));

        if let Some(ref s3) = s3 {
            s3.download_repository(&config, arch)
                .expect("Unable to download files from S3");
        }

        let mut db_repo = guzuta::Repository::new(db_path, repo_signer);
        let mut files_repo = guzuta::Repository::new(files_path, repo_signer);
        let abs = guzuta::Abs::new(&config.name, abs_path);
        db_repo
            .load()
            .expect(&format!("Unable to load database repository from {}",
                             db_repo.path().display()));
        files_repo
            .load()
            .expect(&format!("Unable to load files repository from {}",
                             files_repo.path().display()));

        let package_paths = builder
            .build_package(package_dir.as_path(), repo_dir, &chroot)
            .expect(&format!("Unable to build package in {}",
                             package_dir.as_path().display()));
        for path in &package_paths {
            let package =
                guzuta::Package::load(&path).expect(&format!("Unable to load package {}",
                                                             path.display()));
            db_repo.add(&package);
            files_repo.add(&package);
        }

        abs.add(package_dir.as_path(), &config.srcdest)
            .expect(&format!("Unable to add {} to abs tarball {}",
                             package_dir.as_path().display(),
                             abs.path().display()));
        db_repo
            .save(false)
            .expect(&format!("Unable to save database repository to {}",
                             db_repo.path().display()));
        files_repo
            .save(true)
            .expect(&format!("Unable to save files repository to {}",
                             files_repo.path().display()));

        if let Some(ref s3) = s3 {
            s3.upload_repository(&config, arch, &package_paths)
                .expect("Unable to upload files to S3");
        }
    }
}

fn omakase_remove(args: &clap::ArgMatches) {
    let package_name = args.value_of("PACKAGE_NAME")
        .expect("Unable to get PACKAGE_NAME argument");
    let file = std::fs::File::open(".guzuta.yml").expect("Unable to open .guzuta.yml");
    let config =
        guzuta::omakase::Config::from_reader(file).expect("Unable to load YAML from .guzuta.yml");
    let repo_signer = config
        .repo_key
        .as_ref()
        .map(|key| guzuta::Signer::new(key));
    let repo_signer = repo_signer.as_ref();
    let s3 = config
        .s3
        .as_ref()
        .map(|s3_config| guzuta::omakase::S3::new(s3_config));

    for arch in config.builds.keys() {
        let db_path = config.db_path(arch);
        let files_path = config.files_path(arch);
        let abs_path = config.abs_path(arch);

        if let Some(ref s3) = s3 {
            s3.download_repository(&config, arch)
                .expect("Unable to download files from S3");
        }

        let mut db_repo = guzuta::Repository::new(db_path, repo_signer);
        let mut files_repo = guzuta::Repository::new(files_path, repo_signer);
        let abs = guzuta::Abs::new(&config.name, abs_path);
        db_repo
            .load()
            .expect(&format!("Unable to load database repository from {}",
                             db_repo.path().display()));
        files_repo
            .load()
            .expect(&format!("Unable to load files repository from {}",
                             files_repo.path().display()));

        db_repo.remove(package_name);
        files_repo.remove(package_name);
        abs.remove(package_name)
            .expect(&format!("Unable to remove {} from abs tarball {}",
                             package_name,
                             abs.path().display()));
        db_repo
            .save(false)
            .expect(&format!("Unable to save database repository to {}",
                             db_repo.path().display()));
        files_repo
            .save(true)
            .expect(&format!("Unable to save files repository to {}",
                             files_repo.path().display()));

        if let Some(ref s3) = s3 {
            let paths: Vec<&str> = vec![];
            s3.upload_repository(&config, arch, &paths)
                .expect("Unable to upload files to S3");
        }
    }
}
