use rust_actions::prelude::*;

mod steps;
mod world;

use world::TestWorld;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    RustActions::<TestWorld>::new()
        .features("tests/features")
        .run()
        .await;
}
