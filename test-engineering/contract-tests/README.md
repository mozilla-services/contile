# Contile Contract Tests

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

### client

The `client` directory contains a Python-based test framework for the
contract tests. The HTTP client used in the framework requests tiles from the
MTS and performs checks against the responses. The framework implements response
models for the MTS API.

For more details see the client [README][client_readme]

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

### volumes

The `volumes` directory contains subdirectories which will be mounted as
volumes into the Docker containers used in the contract test suite:

- the `volumes/client` directory contains a YML file which defines every test
scenario that the contract test suite will run
- the `volumes/contile` directory contains files that need to be provided to a
MTS Docker container such as a partner settings file
- the `volumes/partner` directory contains a YML file which defines every
response that the API returns keyed by form-factor and then os-family

## Local Execution

To run the contract tests locally, execute the following from the repository root:

**Build Contile Docker Image**
```shell
docker build -t app:build .
```

**Build Contract Test Docker Images & Execute Tests**
```shell
docker-compose \
  -f test-engineering/contract-tests/docker-compose.yml \
  -p contile-contract-tests \
  up --abort-on-container-exit --build
```

### Import Sorting, Linting, Style Guide Enforcement & Static Type Checking

This project uses [Poetry][poetry] for dependency management. For environment setup it 
is recommended to use [pyenv][pyenv] and [pyenv-virtualenv][pyenv-virtualenv], as they 
work nicely with Poetry.

Project dependencies are listed in the `pyproject.toml` file.
To install the dependencies execute:
```shell
poetry install
```

Contributors to this project are expected to execute the following tools. 
Configurations are set in the `pyproject.toml` and `.flake8` files.

**[isort][isort]**
 ```shell
poetry run isort client partner
 ```
  
**[black][black]**
 ```shell
poetry run black client partner
 ```

**[flake8][flake8]**
 ```shell
poetry run flake8 client partner
 ```

**[mypy][mypy]**
```shell
poetry run mypy client partner
```

## Debugging

The contract testing system is optimized to run within a set of related docker images.

### client

See the `Debugging` section of the client [README][client_readme] 

### Contile

To run the contile service, and it's dependant partner service locally, execute the 
following from the contract-tests root:

```shell
docker-compose run -p 8000:8000 contile
```

Contile runs, by default, on `http://localhost:8000/`.
However, it may be necessary to run the tests outside of docker, in order to debug 
functions or manually verify expected results.

**Fetching Tiles**

Contile will attempt to look up your IP address if you access it using Firefox. 
The easiest way to get a test response back would be to craft a curl request. 
For example (presuming that Contile is running on `http://localhost:8000/v1/tiles`):

```sh
curl -v \
    -H "User-Agent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:10.0) Gecko/20100101 Firefox/91.0'" \
    -H "X-Forwarded-For: '89.160.20.115'" \
    "http://localhost:8000/v1/tiles"
```

### partner

See the `Local Execution` and `Debugging` sections of the partner [README][partner_readme] 


[client_readme]: ./client/README.md
[contract-test-repo]: https://github.com/mozilla-services/contile-integration-tests
[partner_readme]: ./partner/README.md
[sequence_diagram]: sequence_diagram.png
[poetry]: https://python-poetry.org/docs/#installation
[pyenv]: https://github.com/pyenv/pyenv#installation
[pyenv-virtualenv]: https://github.com/pyenv/pyenv-virtualenv#installation
[black]: https://black.readthedocs.io/en/stable/
[flake8]: https://flake8.pycqa.org/en/latest/
[isort]: https://pycqa.github.io/isort/
[mypy]: https://mypy-lang.org/
