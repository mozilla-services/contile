## Requirements

This system uses [Actix](https://actix.rs/) web, and Google Cloud APIs (currently vendored).

## Development Guidelines
Please see the [CONTRIBUTING.md](./CONTRIBUTING.md) docs on commit guidelines and pull request best
practices.

## Versioning
The commit hash of the deployed code is considered its version identifier. The commit hash can be retrieved locally via `git rev-parse HEAD`.

## Development Setup
1. Install Rust. See [rustup.rs](https://rustup.rs/) for how to install on your platform.
2. Compile Contile using `cargo build`.
3. Start a local ADM instance (run from root of Contile repo):
    ```shell
        docker run \
        --env PORT=5000 \
        --env RESPONSES_DIR=/tmp/partner/ \
        --env ACCEPTED_MOBILE_FORM_FACTORS=phone,tablet \
        --env ACCEPTED_DESKTOP_FORM_FACTORS=desktop \
        -v `pwd`/test-engineering/contract/volumes/partner:/tmp/partner \
        -p 5000:5000 \
        mozilla/contile-integration-tests-partner
    ```
4. Start application by running the command below: 
Note that config settings are contained in the  `sa-test.toml` file.  You may change settings [there](sa-test.toml) that pertain to your local development on Contile.
```shell
 #! /bin/bash
RUST_LOG=contile=trace,config=debug \
    cargo run -- --config sa-test.toml #--debug-settings
```
5. Check that the service can accept requests by running:
```shell
curl -v http://localhost:8000/v1/tiles -H "User-Agent:Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:103.0) Gecko/20100101 Firefox/103.0"
```

### Running

Contile is configured via config files and environment variables. For the complete list of available settings, please see [`contile::settings::Settings`](src/settings.rs) (note, you can use `cargo doc --open` to generate documentation.) In general, we have tried to provide sensible default values for most of these,
however you'll need to specify the ADM endpoint URL:

```
CONTILE_ADM_ENDPOINT_URL={Your ADM endpoint} \
    cargo run
```
Please note that the `{}` indicates a variable replacement, and should not be included. For example, a real environment variable would look like: `CONTILE_ADM_ENDPOINT_URL=https://example.com/`