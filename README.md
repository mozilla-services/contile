![Contile graphic](imgs/Contile_title.svg)
# Contile Tile Server

This is the back-end server for the Mozilla Tile Service (MTS).

The goal of this service is to pass tiles from partners along to Firefox for display while ensuring customer privacy and choice as discussed in the [support article "Sponsored tiles on the New Tab page"](https://support.mozilla.org/en-US/kb/sponsor-privacy).

Supports the TopSites feature within Firefox.

See also:

- [In-repo documentation](docs/)
- [Monitoring dashboard](https://earthangel-b40313e5.influxcloud.net/d/oak1zw6Gz/contile-infrastructure) (Mozilla internal)

## Requirements

This system uses [Actix](https://actix.rs/) web, and Google Cloud APIs (currently vendored).

## Setting Up

Contile uses Rust, and requires the latest stable iteration. See
[rustup.rs](https://rustup.rs/) for how to install this application.

Once Rust is installed you can compile using `cargo build`. This will
create a development release.

### Running

Contile is configured via environment variables. To see the complete list of available settings in `contile::settings::Settings` (note, you can use `cargo doc --open` to generate documentation.) In general, we have tried to provide sensible default values for most of these,
however you may need to specify the following:

```
CONTILE_ADM_ENDPOINT_URL={Your ADM endpoint} \
    cargo run
```
Please note that the `{}` indicate a variable replacement and should not be included, for example, a real environmet variable would look like: `CONTILE_ADM_ENDPOINT_URL=https://example.com/`

### Testing
#### Unit Tests

To run Contile's unit tests, run

```cargo test```

This will test everything, except for Google Storage for images. In order to test that, you
will need to include the following:
```
GOOGLE_APPLICATION_CREDENTIALS={path to your credential.json file} \
    CONTILE_TEST_PROJECT={GCP Project name} \
    CONTILE_TEST_BUCKET={GCP Bucket name} \
    cargo test
```

#### Contract Tests

Contract tests are currently run using Docker images. This is so that they can be run as
part of our automated continuous integration (CI) testing. 
See the dedicated [contract-tests README](test-engineering/contract-tests/README.md) for details.

#### Load Tests
Load testing can be run locally or as a part of the deployment process. Please see the [Contile Load (Locust) Tests](test-engineering/load/README.md) for detailed instructions. Local execution does not require any labeling in commit messages. 

For deployment, you have to add a label to the message of the commit that you wish to deploy in the form of: `[load test: (abort|warn)]`. In most cases this will be the merge commit created by merging a GitHub pull request. Abort will prevent deployment should the load testing fail while warn will simply warn via Slack and continue deployment. For detailed specifics on this convention, please see the relevant documentation: [Load Test Readme](test-engineering/load/README.md#opt-in-execution-in-staging-and-production).

#### Releasing to Production
Developers with write access to the Contile repository can initiate a deployment to production after a Pull-Request on the Contile GitHub repository is merged to the `main` branch.

While any developer with write access can trigger the deployment to production, the _expectation_ is that individual(s) who authored and merged the Pull-Request should do so, as they are the ones most familiar with their changes and who can tell, by looking at the data, if anything looks anomalous.

## Why "Contile"?

It's a portmanteau of "Context" and "Tile", which turns out to be the name of [a small village](https://www.google.com/maps/place/Contile/@44.6503701,9.9015688,3a,15y,40.52h,87.97t/data=!3m10!1e1!3m8!1shPkpksIO5_yiJpqYALgcNQ!2e0!6s%2F%2Fgeo3.ggpht.com%2Fcbk%3Fpanoid%3DhPkpksIO5_yiJpqYALgcNQ%26output%3Dthumbnail%26cb_client%3Dmaps_sv.tactile.gps%26thumb%3D2%26w%3D203%26h%3D100%26yaw%3D8.469731%26pitch%3D0%26thumbfov%3D100!7i13312!8i6656!9m2!1b1!2i22!4m5!3m4!1s0x47808736ea28b80d:0xd17ee6c4205c4451!8m2!3d44.650751!4d9.902755) in the Parma region of Italy. So it's pronounced "[kon **tē`** lā](https://translate.google.com/?sl=it&tl=en&text=contile&op=translate)"
