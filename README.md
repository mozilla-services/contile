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

## Development Guidelines
Please see the [CONTRIBUTING.md](./CONTRIBUTING.md) docs on commit guidelines and pull request best
practices.

## Versioning
The commit hash of the deployed code is considered its version identifier. The commit hash can be retrieved locally via `git rev-parse HEAD`.

## Setting Up Your Local Dev Environment

Contile uses Rust, and requires the latest stable iteration. See
[rustup.rs](https://rustup.rs/) for how to install this application.

Once Rust is installed you can compile using `cargo build`. This will 
create a development release.

Additionally, Contile requires a connection to the ADM servers. These use several 
keys (which are specified in the configuration file) to pull results.  The contract
tests include a stub application that can provide ADM-like returns. See the 
[adm_settings_test.json](./adm_settings_test.json) file for reference.

Normally, these configurations run as a part of a `docker-compose` command, but the individual
Docker files can be run by themselves.

To start the ADM stub, you can run the following from the root of the Contile source repository:

```shell
    docker run \
    --env PORT=5000 \
    --env RESPONSES_DIR=/tmp/partner/ \
    --env ACCEPTED_MOBILE_FORM_FACTORS=phone,tablet \
    --env ACCEPTED_DESKTOP_FORM_FACTORS=desktop \
    -v `pwd`/test-engineering/contract-tests/volumes/partner:/tmp/partner \
    -p 5000:5000 \
    mozilla/contile-integration-tests-partner
```

To use the above, you'll need to set the Contile configuration option as follows:
`adm_endpoint_url`  = `http://localhost:5000/tilesp/desktop`. It may be helpful to
create a settings file to contain all of the preferred settings, like the example
of `sa-test.toml` below:

```toml
# sample stand-alone config file "sa-test.toml"

debug=true
port=8000
host="localhost"
human_logs=true
maxmindddb_loc="mmdb/GeoLite2-City-Test.mmdb"

adm_endpoint_url="http://localhost:5000/tilesp/desktop"
adm_defaults='{"image_hosts":["example.com","example.org"],"impression_hosts":["example.com","example.org"],"click_hosts":["example.com","example.org"]}'
adm_settings="test-engineering/contract-tests/volumes/contile/adm_settings.json"
adm_partner_id="partner_id_test"
adm_sub1="sub1_test"
adm_has_legacy_image='["Example ORG","Example COM"]'
adm_timeout=2
tiles_ttl=0
```

You can then start the local dev application by running: 
```shell
 #! /bin/bash
RUST_LOG=contile=trace,config=debug \
    cargo run -- --config sa-test.toml #--debug-settings
```

If you used the sample `sa-test.toml` configuration file as defined above, you should
be able to make requests to the dev server like so:

```shell
curl -v http://localhost:8000/v1/tiles -H "User-Agent:Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:103.0) Gecko/20100101 Firefox/103.0"
```

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

### Deployment

#### Releasing to Production

Developers with write access to the Contile repository will initiate a deployment to production when
a pull request on the Contile GitHub repository is merged to the `main` branch. It is recommended to
merge pull requests during hours when the majority of Contile contributors are online.

While any developer with write access can trigger the deployment to production, the _expectation_ is
that the individual(s) who authored and merged the pull request should do so, as they are the ones
most familiar with their changes and who can tell, by looking at the data, if anything looks
anomalous. Developers **must** monitor the [Contile Infrastructure][contile_infrastructure]
dashboard for any anomaly, for example significant changes in HTTP response codes, increase in
latency, cpu/memory usage (most things under the 'Metrics' heading).

[contile_infrastructure]: https://earthangel-b40313e5.influxcloud.net/d/oak1zw6Gz/contile-infrastructure?orgId=1&refresh=1m

#### What to do if production breaks?
If your latest release causes problems and needs to be rolled back:
don't panic and follow the instructions below:

1. Depending on the severity of the problem, decide if this warrants [kicking off an incident][incident_docs];
2. Identify the problematic commit, as it may not necessarily be the latest one!
3. Revert the problematic commit, merge that into GitHub,
   then [deploy the revert commit to production](#releasing-to-production).
   - If a fix can be identified in a relatively short time,
     then you may submit a fix, rather than reverting the problematic commit.


##### Locate problematic commit via "git bisect"
If you are not sure about which commit broke production, you can use `git bisect` to locate the problematic commit as follows:

```sh
# Start the bisect session.
$ git bisect start

# Flag a bad commit, usually you can set it to the latest commit as it's broken
# in production.
$ git bisect bad <commit-hash-for-a-bad-commit>

# Flag a good commit, this can be set to the last known good commit.
# You can use an old commit if you're still unsure, bisect will perform binary
# searches anyway.
$ git bisect good <commit-hash-for-a-good-commit>

# Git will check out a commit in the middle and then you can test it to see if
# the issue is still reproducible. If yes, flag it "bad", otherwise flag it
# "good". Git will use that as input to make the next move until it locates the
# problematic commit.
$ git bisect [bad|good]

# End the bisect session when you're done.
$ git bisect reset
```

[incident_docs]: https://mozilla-hub.atlassian.net/wiki/spaces/MIR/overview

## Why "Contile"?

It's a portmanteau of "Context" and "Tile", which turns out to be the name of [a small village](https://www.google.com/maps/place/Contile/@44.6503701,9.9015688,3a,15y,40.52h,87.97t/data=!3m10!1e1!3m8!1shPkpksIO5_yiJpqYALgcNQ!2e0!6s%2F%2Fgeo3.ggpht.com%2Fcbk%3Fpanoid%3DhPkpksIO5_yiJpqYALgcNQ%26output%3Dthumbnail%26cb_client%3Dmaps_sv.tactile.gps%26thumb%3D2%26w%3D203%26h%3D100%26yaw%3D8.469731%26pitch%3D0%26thumbfov%3D100!7i13312!8i6656!9m2!1b1!2i22!4m5!3m4!1s0x47808736ea28b80d:0xd17ee6c4205c4451!8m2!3d44.650751!4d9.902755) in the Parma region of Italy. So it's pronounced "[kon **tē`** lā](https://translate.google.com/?sl=it&tl=en&text=contile&op=translate)"
