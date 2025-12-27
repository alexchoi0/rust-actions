use rust_actions::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(World)]
#[world(init = Self::setup)]
pub struct TestWorld {
    pub rng: SeededRng,
    pub users: Vec<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
}

impl TestWorld {
    pub async fn setup() -> Result<Self> {
        Ok(Self {
            rng: SeededRng::new(),
            users: Vec::new(),
        })
    }
}
