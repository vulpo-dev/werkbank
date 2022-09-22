use ::clap::ArgMatches;
use std::env;

pub mod args;

pub fn run_migration(matches: &ArgMatches) -> bool {
    env::var("VULPO_RUN_MIGRATIONS").is_ok()
        || matches.subcommand_matches("init").is_some()
        || matches.subcommand_matches("migrations").is_some()
}

pub fn run_server(matches: &ArgMatches) -> Option<&ArgMatches> {
    matches.subcommand_matches("server")
}

pub fn get_config_dir(dir: Option<&String>) -> String {
    dir.map(|val| val.clone().to_owned())
        .unwrap_or_else(|| String::from("Vulpo.toml"))
}
