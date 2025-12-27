use rust_actions::prelude::*;
use rust_actions_example::TestWorld;

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn run_features() {
    RustActions::<TestWorld>::new()
        .features("tests/features")
        .run()
        .await;
}
