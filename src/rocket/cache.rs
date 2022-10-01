use async_trait::async_trait;
use figment::providers::Env;
use figment::Figment;
use futures::lock::Mutex;
use lru::LruCache;
use redis::AsyncCommands;
use redis::Client;
use retainer;
use rocket::fairing::AdHoc;
use rocket::fairing::Fairing;
use rocket::request::Outcome;
use rocket::request::{FromRequest, Request};
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio;
use tracing::info;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CacheConfig {
    pub url: Option<String>,
    pub cache_size: Option<NonZeroUsize>,
    pub off: Option<bool>,
}

pub fn get_cache_config(figment: &Figment) -> CacheConfig {
    figment
        .clone()
        .select("cache")
        .merge(Env::prefixed("VULPO_CACHE_").global())
        .extract::<CacheConfig>()
        .unwrap_or_else(|_| CacheConfig {
            url: None,
            cache_size: None,
            off: Some(false),
        })
}

#[async_trait]
pub trait CacheProvider {
    async fn get(&self, key: &Path) -> Option<String>;
    async fn delete(&self, key: &Path) -> Option<()>;
    async fn set(&self, key: &Path, value: &str) -> Option<()>;

    async fn set_ex(&self, key: &Path, value: &str, ttl: Duration) -> Option<()>;
}

pub struct Cache(Arc<dyn CacheProvider + Sync + Send>);

impl Deref for Cache {
    type Target = Arc<dyn CacheProvider + Sync + Send>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Cache {
    pub fn fairing(figment: &Figment) -> impl Fairing {
        let config = get_cache_config(&figment);

        AdHoc::on_ignite("Add Cache", move |rocket| async move {
            if let Some(off) = config.off {
                if off {
                    info!("Cache: Off");
                    return rocket;
                }
            }

            if let Some(url) = config.url {
                info!("Cache: Use redis");
                let cache = RedisProvider::new(&url);
                return rocket.manage(Arc::new(cache));
            }

            info!("Cache: Default use memory");
            let cache = MemoryProvider::new(config.cache_size);
            return rocket.manage(Arc::new(cache));
        })
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Cache {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        if let Some(redis) = request.rocket().state::<Arc<RedisProvider>>() {
            return Outcome::Success(Cache(redis.clone()));
        }

        if let Some(memory) = request.rocket().state::<Arc<MemoryProvider>>() {
            return Outcome::Success(Cache(memory.clone()));
        }

        Outcome::Success(Cache(Arc::new(NoCacheProvider)))
    }
}

pub struct RedisProvider {
    client: Client,
}

impl RedisProvider {
    pub fn new(connection: &str) -> RedisProvider {
        let client = redis::Client::open(connection).unwrap();
        RedisProvider { client }
    }
}

#[async_trait]
impl CacheProvider for RedisProvider {
    async fn get(&self, key: &Path) -> Option<String> {
        let mut con = self.client.get_async_connection().await.ok()?;
        con.get(key.to_str()).await.ok()
    }

    async fn delete(&self, key: &Path) -> Option<()> {
        let mut con = self.client.get_async_connection().await.ok()?;
        con.del(key.to_str()).await.ok()
    }

    async fn set(&self, key: &Path, value: &str) -> Option<()> {
        let mut con = self.client.get_async_connection().await.ok()?;
        let _: String = con.set(key.to_str(), value).await.ok()?;
        Some(())
    }

    async fn set_ex(&self, key: &Path, value: &str, ttl: Duration) -> Option<()> {
        let mut con = self.client.get_async_connection().await.ok()?;
        let ttl: usize = ttl.as_millis().try_into().unwrap_or(usize::MAX);
        let _: String = con.set_ex(key.to_str(), value, ttl).await.ok()?;
        Some(())
    }
}

pub struct MemoryProvider {
    lru: Arc<Mutex<LruCache<String, String>>>,
    retainer: Arc<retainer::Cache<String, String>>,
}

impl MemoryProvider {
    pub fn new(cache_size: Option<NonZeroUsize>) -> MemoryProvider {
        let lru = if let Some(size) = cache_size {
            LruCache::new(size)
        } else {
            LruCache::unbounded()
        };

        let retainer_cache = Arc::new(retainer::Cache::new());
        let clone = retainer_cache.clone();

        tokio::spawn(async move { clone.monitor(4, 0.25, Duration::from_secs(3)).await });

        let lru = Arc::new(Mutex::new(lru));
        MemoryProvider {
            lru,
            retainer: retainer_cache,
        }
    }
}

#[async_trait]
impl CacheProvider for MemoryProvider {
    async fn get(&self, key: &Path) -> Option<String> {
        let key = key.to_str()?;
        let lru = Arc::clone(&self.lru);
        let mut store = lru.lock().await;

        if let Some(value) = store.get(key) {
            return Some(value.to_owned());
        }

        let retainer = Arc::clone(&self.retainer);
        if let Some(value) = retainer.get(&key.to_string()).await {
            return Some(value.to_owned());
        }

        None
    }

    async fn delete(&self, key: &Path) -> Option<()> {
        let key = key.to_str()?;
        let lru = Arc::clone(&self.lru);
        let mut store = lru.lock().await;
        store.pop(key)?;
        Some(())
    }

    async fn set(&self, key: &Path, value: &str) -> Option<()> {
        let key = key.to_str()?;
        let lru = Arc::clone(&self.lru);
        let mut store = lru.lock().await;
        store.put(key.to_string(), value.to_string());
        Some(())
    }

    async fn set_ex(&self, key: &Path, value: &str, ttl: Duration) -> Option<()> {
        let key = key.to_str()?;
        let retainer = Arc::clone(&self.retainer);
        let ttl: u64 = ttl.as_millis().try_into().unwrap_or(u64::MAX);
        retainer
            .insert(key.to_string(), value.to_string(), ttl)
            .await?;
        Some(())
    }
}

struct NoCacheProvider;

#[async_trait]
impl CacheProvider for NoCacheProvider {
    async fn get(&self, _key: &Path) -> Option<String> {
        None
    }

    async fn delete(&self, _key: &Path) -> Option<()> {
        Some(())
    }

    async fn set(&self, _key: &Path, _value: &str) -> Option<()> {
        Some(())
    }

    async fn set_ex(&self, _key: &Path, _value: &str, _ttl: Duration) -> Option<()> {
        Some(())
    }
}
