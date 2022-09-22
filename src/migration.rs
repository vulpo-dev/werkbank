use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::Row;
use std::str::FromStr;
use url::Url;

pub async fn create_db(database_url: &str) {
    let mut url = Url::parse(database_url).expect("Invalid database url");

    let database_url = url.clone();
    let database = database_url
        .path_segments()
        .and_then(|mut segments| segments.nth(0))
        .expect("database name");

    url.set_path("postgres");

    let options = PgConnectOptions::from_str(&url.to_string())
        .expect("valid db connection string")
        .to_owned();

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("Failed to connect");

    let row = sqlx::query(
        "
        select count(*)
          from pg_catalog.pg_database
         where datname = $1
    ",
    )
    .bind(database)
    .fetch_one(&pool)
    .await
    .expect("faild to run query");

    if row.get::<i64, &str>("count") == 0 {
        let query = format!("create database {}", database);
        sqlx::query(&query)
            .execute(&pool)
            .await
            .expect("faild to create database");
        println!("Database({}) created", database);
    } else {
        println!("Database({}) exists", database);
    }
}
