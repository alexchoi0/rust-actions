# rust-actions

A BDD testing framework for Rust using GitHub Actions YAML syntax instead of Gherkin.

[![Crates.io](https://img.shields.io/crates/v/rust-actions.svg)](https://crates.io/crates/rust-actions)
[![Documentation](https://docs.rs/rust-actions/badge.svg)](https://docs.rs/rust-actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Features

- **GitHub Actions YAML syntax** - Write tests in familiar YAML format
- **Typed step definitions** - Full Rust type safety with `#[derive(Args)]` and `#[derive(Outputs)]`
- **Auto-registration** - Steps are automatically discovered via `#[step("name")]` attribute
- **Inline assertions** - Pre and post assertions with expression-based syntax
- **Deterministic testing** - Seeded RNG for reproducible tests
- **Testcontainers support** - Built-in Docker container management

## Installation

Add to your `Cargo.toml`:

```toml
[dev-dependencies]
rust-actions = "0.1"
tokio = { version = "1", features = ["full", "test-util"] }
```

## Quick Start

### 1. Define your World

```rust
// tests/world.rs
use rust_actions::prelude::*;

#[derive(World)]
pub struct TestWorld {
    pub users: Vec<User>,
    pub rng: SeededRng,
}

impl TestWorld {
    pub async fn setup() -> Result<Self> {
        Ok(Self {
            users: vec![],
            rng: SeededRng::new(),
        })
    }
}

pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
}
```

### 2. Define your Steps

```rust
// tests/steps.rs
use rust_actions::prelude::*;
use crate::world::{TestWorld, User};

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

    world.users.push(User {
        id: id.clone(),
        username: args.username.clone(),
        email: args.email,
    });

    Ok(UserOutput { id, username: args.username })
}
```

### 3. Write your Feature File

```yaml
# tests/features/user.yaml
name: User Management

scenarios:
  - name: Create a new user
    steps:
      - name: Create user Alice
        id: alice
        uses: user/create
        with:
          username: alice
          email: alice@example.com
        assert-after:
          - ${{ outputs.id != "" }}
          - ${{ outputs.username == "alice" }}
```

### 4. Run the Tests

```rust
// tests/main.rs
use rust_actions::prelude::*;

mod steps;
mod world;

use world::TestWorld;

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn run_features() {
    RustActions::<TestWorld>::new()
        .features("tests/features")
        .run()
        .await;
}
```

## YAML Syntax

### Basic Structure

```yaml
name: Feature Name

env:
  DB_URL: postgres://localhost/test

containers:
  postgres: postgres:15
  redis: redis:7

scenarios:
  - name: Scenario Name
    steps:
      - name: Step description
        id: step_id           # Optional: reference outputs later
        uses: step/name       # Required: step to execute
        with:                 # Optional: step arguments
          arg1: value1
          arg2: ${{ steps.previous.outputs.field }}
        continue-on-error: true  # Optional: don't fail on error
        assert-before:        # Optional: assertions before step
          - ${{ env.DB_URL != "" }}
        assert-after:         # Optional: assertions after step
          - ${{ outputs.id != "" }}
```

### Expression Syntax

Access data using `${{ }}` expressions:

```yaml
# Environment variables
${{ env.DB_URL }}

# Previous step outputs
${{ steps.user.outputs.id }}

# Container info
${{ containers.postgres.url }}
${{ containers.postgres.host }}
${{ containers.postgres.port }}

# Current step outputs (in assert-after only)
${{ outputs.id }}
```

### Assertions

Inline assertions support comparison operators and object matching:

```yaml
assert-after:
  # Scalar comparisons
  - ${{ outputs.id != "" }}
  - ${{ outputs.count > 0 }}
  - ${{ outputs.status == "active" }}

  # Object partial matching
  - '${{ outputs contains { "username": "alice" } }}'

  # Array contains
  - '${{ outputs.tags contains "admin" }}'
  - '${{ outputs.users contains { "name": "bob" } }}'

  # Full object equality
  - '${{ outputs == { "id": "123", "name": "alice" } }}'
```

**Supported operators:**
- Comparison: `==`, `!=`, `>`, `<`, `>=`, `<=`
- Subset matching: `contains`

## Step Definitions

### Basic Step

```rust
#[step("my/step")]
async fn my_step(world: &mut TestWorld, args: MyArgs) -> Result<MyOutput> {
    // Implementation
}
```

### Args and Outputs

```rust
#[derive(Deserialize, Args)]
struct MyArgs {
    required_field: String,
    #[serde(default)]
    optional_field: Option<String>,
}

#[derive(Serialize, Outputs)]
struct MyOutput {
    id: String,
    created_at: String,
}
```

### Step without Args

```rust
#[step("simple/step")]
async fn simple_step(world: &mut TestWorld) -> Result<()> {
    // No args, no outputs
    Ok(())
}
```

## Determinism

rust-actions provides helpers for deterministic testing:

### SeededRng

```rust
use rust_actions::prelude::*;

#[derive(World)]
pub struct TestWorld {
    pub rng: SeededRng,
}

impl TestWorld {
    pub async fn setup() -> Result<Self> {
        Ok(Self {
            rng: SeededRng::new(), // Seeded from scenario name
        })
    }
}

#[step("user/create")]
async fn create_user(world: &mut TestWorld, args: Args) -> Result<Output> {
    let id = world.rng.next_uuid();        // Deterministic UUID
    let token = world.rng.next_string(32); // Deterministic string
    // ...
}
```

### Time Control

Uses tokio's `test-util` for time manipulation:

```rust
#[step("time/advance")]
async fn advance_time(_world: &mut TestWorld, args: TimeArgs) -> Result<()> {
    tokio::time::advance(args.duration).await;
    Ok(())
}
```

## Output

```
Feature: User Management
  ✓ Create a new user (5ms)
    ✓ Create user Alice

1 scenarios ✓ (1 passed)
1 steps (1 passed, 0 failed)
```

## License

MIT
