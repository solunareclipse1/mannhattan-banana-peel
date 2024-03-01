use std::sync::Arc;
use rive_gateway::*;
use rive_cache_inmemory::*;
use rive_models::authentication::Authentication;
use rive_models::event::ServerEvent;

#[derive(Debug, Clone)]
pub struct Rive {
    pub http: rive_http::Client,
    pub gateway: Gateway,
    //pub autumn: rive_autumn::Client,
    pub cache: Arc<InMemoryCache>,
}

impl Rive {
    /// Creates a new [`Rive`].
    // TODO: make a separated error struct instead of gateway error exclusively?
    // i mean that's kinda crappy isn't it? ------------>  VVVVVVVVVVVVVVVVVVV
    pub async fn new(auth: Authentication) -> Result<Self, rive_gateway::Error> {
        let http = rive_http::Client::new(auth.clone());
        let gateway = Gateway::connect(auth).await?;
        //let autumn = rive_autumn::Client::new();
        let cache = Arc::new(InMemoryCache::new());

        Ok(Self {
            http,
            gateway,
            //autumn,
            cache,
        })
    }

    /// Handle an incoming event.
    pub fn update(&self, event: &ServerEvent) {
        self.cache.update(event);
    }
}

