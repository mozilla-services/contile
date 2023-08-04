![Contile graphic](imgs/Contile_title.svg)
# Contile Tile Server

This is the back-end server for the Mozilla Tile Service (MTS).

The goal of this service is to pass tiles from partners along to Firefox for display while ensuring customer privacy and choice as discussed in the [support article "Sponsored tiles on the New Tab page"](https://support.mozilla.org/en-US/kb/sponsor-privacy).

Supports the TopSites feature within Firefox.

See also:

- [Documentation](https://mozilla-services.github.io/contile/)
- [Monitoring dashboard](https://earthangel-b40313e5.influxcloud.net/d/oak1zw6Gz/contile-infrastructure) (Mozilla internal)



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
See the dedicated [contract tests README](test-engineering/contract/README.md) for details.

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
