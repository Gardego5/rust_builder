# Building a lambda

## Disclaimers

So - some things I've found...

1. Rust is fast, and memory safe, but performance shouldn't be the only reason you use rust.
2. The Rust

## Setup

```sh
# Initialize the lambda project.
cargo lambda new image_lambda_2;

# Install some crates (dependencies).

# Web framework, makes it much more ergonomic to interact with http requests.
cargo add axum;
# Negotiate Accept header to send appropriate response.
cargo add accept_header mime;
# Serialize / Deserialize (read) Query params, headers, etc....
# `derive` lets us just annotate a struct and it will then be automatically deserialized.
cargo add serde --features derive;
# Interact with AWS.
cargo add aws_config aws_sdk_s3;
# Resize images.
cargo add image;
```

Show Cargo.toml

```sh
# Build it yo!
cargo lambda build \
  --release \
  --arm64 \
  --lambda-dir ../../bin;
```

Show build script
Builds all lambdas in `./resources/` directory
Likely better ways to do multiple lambdas, eg. you can have cargo output multiple binaries.

## Code

Show main.rs

We are given a main function, and a handler fuction.

### `#[tokio::main]`

We designate this as the main entry point for the lambda.

When our lambda is invoked, this gets called _Once_

We want to do all our set inside here so it can be reused.

```rust
#[tokio::main]
async fn main() -> Result<(), Error> {}
```

We want to do as much as possible during the cold start.

Create struct to store the warmed up lambda context.

```rust
struct WarmContext {
  s3: aws_sdk_s3::Client,
  bucket_name: String, // Environment Variable
}
```
