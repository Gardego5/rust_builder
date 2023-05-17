# Building a lambda

## Setup

```sh
# Initialize the lambda project.
cargo lambda new image_lambda_2;

# Install some crates (dependencies).
cargo add axum aws_config aws_sdk_s3 accept_header image mime;
cargo add serde --features derive;
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

```rust
#[tokio::main]
async fn main() -> Result<(), Error> {}
```

We designate this as the main entry point for the binary.

`bootstrap` is the generated executable that will be run by the lambda runtime.

When our lambda is invoked, this gets called _Once_

Then, each invocation will call the hanlder function (service).

We want to do all our setup inside here so it can be reused.

`async fn function_handler` is the lambda function, called on each invocation.

## `lambda_http` vs `axum`

lambda_http crate adds http types for handling REST APIs, HTTP APIs, Lambda Function URLs

We're going to use axum, which simplifies deserializing query string parameters, headers, path, other things.

delete existing
