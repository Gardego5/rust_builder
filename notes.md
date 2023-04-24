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
cargo add aws_config aws_sdk_s3 accept_header mime serde_json mime;
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
  bucket_name: String, // Environment Variable
  s3_client: aws_sdk_s3::Client,
}
```

Parse Environment Variables
Create S3 Client

Change our `function_handler` to accept a ctx
**don't use reference**

```rust
async fn function_handler(event: Request, ctx: WarmContext) ->
  Result<Response<Body>, Error> {}
```

```rust initialize context
let config = aws_config::from_env().load().await; // Await lets there be a break point for tokio

let ctx = WarmContext {
  s3_client: aws_sdk_s3::Client::new(&config),
  bucket_name: std::env::var("BUCKET_NAME")?, // try... if it fails, early return.
}
```

function returns result, nothing or Error.
Special "boxed" error, accepts most errors.
If something goes wrong, it will log to cloudfront.

```rust update function_handler call
run(service_fn(|req| function_handler(req, ctx))).await
```

Updated.
LSP Error because:

> main.rs(43, 5): the requirement to implement `FnMut` derives from here \
> main.rs(43, 48): closure is `FnOnce` because it moves the variable `ctx` out of its environment

`FnOnce` consumes `ctx`, we don't need to use it up, we just need to observe it.
We just need a _reference!_

### function handler

What it will do:

1. 2 Required query params \
   both need to be a number.

   - `width`
   - `height`

2. Load the image from s3.

3. Resize it (as specified from the query params)

4. Respond with the generated image.
