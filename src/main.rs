extern crate clap;
extern crate env_logger;
extern crate guzuta;

fn main() {
    env_logger::init().unwrap();

    let app = clap::App::new("guzuta")
        .version("0.0.0")
        .about("Custom repository manager for ArchLinux pacman")
        .subcommand(clap::SubCommand::with_name("repo-add")
            .about("Add PACKAGE_PATH to DB_PATH")
            .arg(clap::Arg::with_name("repo-key")
                .long("repo-key")
                .help("GPG key to sign repository database"))
            .arg(clap::Arg::with_name("PACKAGE_PATH")
                .required(true)
                .help("Path to package to be added"))
            .arg(clap::Arg::with_name("DB_PATH")
                .required(true)
                .help("Path to repository database")));
    let matches = app.clone().get_matches();

    run_subcommand(matches.subcommand());
}

fn run_subcommand(subcommand: (&str, Option<&clap::ArgMatches>)) {
    match subcommand {
        ("repo-add", Some(repo_add_command)) => {
            repo_add(repo_add_command);
        }
        _ => {
            panic!("Unknown subcommand");
        }
    }
}

fn repo_add(args: &clap::ArgMatches) {
    let signer = args.value_of("repo-key").map(|key| guzuta::Signer::new(key.to_owned()));
    let package = guzuta::Package::load(&args.value_of("PACKAGE_PATH").unwrap());
    let mut repository = guzuta::Repository::new(args.value_of("DB_PATH").unwrap().to_owned(),
                                                 signer);

    repository.load();
    repository.add(&package);
    repository.save();
}
