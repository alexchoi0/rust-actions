use rust_actions::prelude::*;
use crate::world::{TestWorld, User};
use serde::Deserialize;

#[derive(Deserialize, Args)]
pub struct CreateUserArgs {
    pub username: String,
    pub email: String,
}

#[derive(Serialize, Outputs)]
pub struct UserOutput {
    pub id: String,
    pub username: String,
}

#[step("user/create")]
pub async fn create_user(world: &mut TestWorld, args: CreateUserArgs) -> Result<UserOutput> {
    let id = world.rng.next_uuid().to_string();

    let user = User {
        id: id.clone(),
        username: args.username.clone(),
        email: args.email,
    };

    world.users.push(user);

    Ok(UserOutput {
        id,
        username: args.username,
    })
}
