# contile-contract-tests

This directory contains the automated contract test suite for the Mozilla Tile 
Service (MTS). Passing contract tests are a prerequisite for moving to the next 
phase in the rollout plan. The contract test framework was originally developed 
in isolation, see [contile-integration-tests][contract-test-repo].

## Overview

The contract test suite is designed to be set up as a `docker-compose` CI
workflow. The following sections as well as the sequence diagram below describe 
the individual components of the suite.

**Test Scenario: success_tiles_cached_for_identical_proxy_params** 
![Sequence diagram of the integration tests][sequence_diagram]

To run the contract tests locally, execute the following from the repository root:

```text
u
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

For more details see the partner [README][partner_readme]

### client

The `client` directory contains a Python-based test framework for the
contract tests. The HTTP client used in the framework requests tiles from the
MTS and performs checks against the responses. The framework implements response
models for the MTS API.

For more details see the client [README][client_readme]

### volumes

The `volumes` directory contains subdirectories which will be mounted as
volumes into the Docker containers used in the contract test suite:

- the `volumes/partner` directory contains a YML file which defines every
response that the API returns keyed by form-factor and then os-family
- the `volumes/contile` directory contains files that need to be provided to a
MTS Docker container such as a partner settings file
- the `volumes/client` directory contains a YML file which defines every test
scenario that the contract test suite will run

[client_readme]: ./client/README.md
[contract-test-repo]: https://github.com/mozilla-services/contile-integration-tests
[partner_readme]: ./partner/README.md
[sequence_diagram]: sequence_diagram.png
