use clap::{Arg, Command};

pub fn version() -> Arg {
    Arg::new("version")
        .short('v')
        .long("version")
        .required(false)
        .value_name("VERSION")
        .num_args(0)
}

pub fn config() -> Arg {
    Arg::new("config")
        .short('c')
        .long("config")
        .required(false)
        .value_name("CONFIG")
        .num_args(1)
}

pub fn server() -> Command {
    Command::new("server")
        .about("start server")
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .required(false)
                .value_name("PORT")
                .num_args(1),
        )
        .arg(
            Arg::new("run-migrations")
                .long("run-migrations")
                .required(false)
                .value_name("RUN_MIGRATION")
                .num_args(0),
        )
}

pub fn migrations() -> Command {
    Command::new("migrations").about("run migrations")
}

pub fn init() -> Command {
    Command::new("init").about("initialize the server")
}
