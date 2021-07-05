# contile-integration-tests

This repository contains the source code for our automated integration test
suite for the [Mozilla Tile Service (MTS)][mts]. Passing integration tests are a
prerequisite for moving to the next phase in the rollout plan.

## Overview

The integration test suite is designed to be set up as a `docker-compose` CI
workflow in the MTS repository. The following sections as well as the sequence
diagram below describe the individual components of the suite.

![Sequence diagram of the integration tests][sequence_diagram]

This test architecture enables Contextual Services engineers to configure test
scenarios to run in the MTS repository rather than in this repository. This
means the MTS code and the integration tests suite should always be in sync.

You can run a set of example integration tests as follows:

```text
docker compose up --abort-on-container-exit --build
```

### partner

The `partner` directory contains a Python-based web service. The HTTP API of
this service implements the API specification of the partner API that MTS
connects to when requesting tiles to pass along to Firefox for display.

When a client sends a request to the MTS, information about the client's form
factor and OS family are parsed from the `User-Agent` header. Then, when the MTS
sends a request to the partner API the form factor and OS family information is
included in the query parameters. We leverage this behavior to map requests from
a client to specific responses from the partner API. We can control not only the
response content, but also the response status code, response headers and even
delay the response for a period of time, which allows us to effectively test the
MTS.

### client

The `client` directory contains a Python-based test framework for the
integration tests. The HTTP client used in the framework requests tiles from the
MTS and performs checks against the responses. The framework implements response
models for the MTS API.

### volumes

The `volumes` directory contains subdirectories which will be mounted as
volumes into the Docker containers used in the integration test suite:

- the `volumes/partner` directory contains a YML file which defines every
response that the API returns keyed by form-factor and then os-family
- the `volumes/contile` directory contains files that need to be provided to a
MTS Docker container such as a partner settings file
- the `volumes/client` directory contains a YML file which defines every test
scenario that the integration test suite will run

The files in the `volumes` directory  as well as the `docker-compose.yml` file
in this repository are checked in only for local development and demonstration
purposes.


[mts]: https://github.com/mozilla-services/contile
[sequence_diagram]: /sequence_diagram.png
