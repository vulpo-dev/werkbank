use figment::providers::Env;
use figment::value::{Dict, Map};
use figment::{Error, Figment, Metadata, Profile, Provider};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Status;
use rocket::http::{ContentType, Header, Method};
use rocket::{Request, Response};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
pub struct Cors {
    origin: Arc<String>,
    methods: Arc<String>,
    headers: Arc<String>,
}

impl Default for Cors {
    fn default() -> Self {
        Cors {
            origin: Arc::new("*".to_string()),
            methods: Arc::new("POST, GET, OPTIONS".to_string()),
            headers: Arc::new("Content-Type, Vulpo-Project, Authorization".to_string()),
        }
    }
}

impl Cors {
    pub fn from_figment(figment: &Figment) -> Cors {
        Figment::from(Cors::default())
            .merge(figment.clone().select("cors"))
            .merge(Env::prefixed("CORS_").global())
            .extract::<Cors>()
            .expect("Valid CORS config")
    }
}

impl Provider for Cors {
    fn metadata(&self) -> Metadata {
        Metadata::named("Cors Config")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        figment::providers::Serialized::defaults(Cors::default()).data()
    }

    fn profile(&self) -> Option<Profile> {
        None
    }
}

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to requests",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
        response.set_header(Header::new(
            "Access-Control-Allow-Origin",
            self.origin.clone().to_string(),
        ));

        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            self.methods.clone().to_string(),
        ));
        response.set_header(Header::new(
            "Access-Control-Allow-Headers",
            self.headers.clone().to_string(),
        ));
        response.set_header(Header::new("Access-Control-Allow-Credentials", "false"));

        if request.method() == Method::Options {
            response.set_header(ContentType::Plain);
            response.set_sized_body(0, Cursor::new(""));
            response.set_status(Status::Ok);
        }
    }
}
