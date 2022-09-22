use std::ops::Deref;
use std::str::FromStr;

use figment::{providers::Env, Figment};
use rocket::fairing::{AdHoc, Fairing};
use rocket::http::Status;
use rocket::request::Outcome;
use rocket::request::{FromRequest, Request};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use sqlx::PgPool;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DbConfig {
    pub database_url: Option<String>,
    pub database_pool_size: Option<u32>,
    pub cache_url: Option<String>,
}

pub fn get_db_config(figment: &Figment) -> DbConfig {
    figment
        .clone()
        .select("database")
        .merge(Env::prefixed("VULPO_").global())
        .extract::<DbConfig>()
        .expect("Invalid Database config")
}

pub fn create_pool(config: &Figment) -> impl Fairing {
    let config = get_db_config(&config);
    let max_connections = config.database_pool_size.unwrap_or(100);
    let database_url = config
        .database_url
        .or_else(|| Env::var("DATABASE_URL"))
        .unwrap_or("postgres://postgres:postgres@localhost:5432/postgres".to_string());

    AdHoc::on_ignite("Add DB pool", move |rocket| async move {
        let options = PgConnectOptions::from_str(&database_url)
            .expect("valid db connection string")
            .disable_statement_logging()
            .to_owned();

        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect_with(options)
            .await
            .expect("Failed to connect");

        rocket.manage(pool)
    })
}

pub struct Db(PgPool);

impl Deref for Db {
    type Target = PgPool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Db {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.rocket().state::<PgPool>() {
            None => Outcome::Failure((Status::InternalServerError, ())),
            Some(pool) => Outcome::Success(Db(pool.to_owned())),
        }
    }
}
