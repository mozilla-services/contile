# Contile Contract Tests

This directory contains the automated contract test suite for the Mozilla Tile Service (MTS).
Passing contract tests are a prerequisite for deployment. The contract test framework was
originally developed in isolation, see [contile-integration-tests][1].

## Overview

The contract test suite is designed to be set up as a `docker-compose` CI workflow. The following
sections as well as the sequence diagram below describe the individual components of the suite.

**Test Scenario: success_tiles_cached_for_identical_proxy_params**
![Sequence diagram of the integration tests][2]
The sequence diagram was created with [miro][3]

### client

The `client` directory contains a Python-based test framework for the contract tests. The HTTP
client used in the framework requests tiles from the MTS and performs checks against the responses.
The framework implements response models for the MTS API.

For more details see the client [README][4]

### partner

The `partner` directory contains a Python-based web service. The HTTP API of this service implements
the API specification of the partner API that MTS connects to when requesting tiles to pass along to
Firefox for display.

When a client sends a request to the MTS, information about the client's form factor and OS family
are parsed from the `User-Agent` header. Then, when the MTS sends a request to the partner API the
form factor and OS family information is included in the query parameters. We use this behavior
to map requests from a client to specific responses from the partner API. We can control the
response content, the response status code, the response headers and can also delay the response
for a period of time, which allows us to effectively test the MTS.

For more details see the partner [README][5]

### volumes

The `volumes` directory contains subdirectories which will be mounted as volumes into the Docker
containers used in the contract test suite:

* the [volumes/client][25] directory contains YML files which define every test scenario that the
  contract test suite will run
* the [volumes/contile][26] directory contains files that need to be provided to a MTS Docker 
  container such as a partner settings file
* the [volumes/partner][27] directory contains a YML file which defines every response that the API
  returns keyed by form-factor and then os-family

## Local Execution

To run the contract tests locally, execute the following from the repository root:

**Build Contile Docker Image**

```shell
docker build -t app:build .
```

**Build Contract Test Docker Images & Execute Tests**

```shell
docker-compose \
  -f test-engineering/contract/docker-compose.yml \
  -p contile-contract-tests \
  up --abort-on-container-exit --build
```

### Import Sorting, Linting, Style Guide Enforcement & Static Type Checking

This project uses [Poetry][6] for dependency management. For environment setup, it is recommended to
use [pyenv][7] and [pyenv-virtualenv][8], as they work nicely with Poetry.

Project dependencies are listed in the `pyproject.toml` file.
To install the dependencies execute:

```shell
poetry install
```

Contributors to this project are expected to execute the following tools.
Configurations are set in the `pyproject.toml` and `.flake8` files.

**[isort][9]**

 ```shell
poetry run isort client partner
 ```

**[black][10]**

 ```shell
poetry run black client partner
 ```

**[flake8][11]**

 ```shell
poetry run flake8 client partner
 ```

**[mypy][12]**

```shell
poetry run mypy client partner
```

## Debugging

The contract testing system is optimized to run within a set of related docker images.

### client

See the `Debugging` section of the client [README][4]

### Contile

To run the contile service, and it's dependent partner service locally, execute the following from
the contract tests root:

```shell
docker-compose run -p 8000:8000 contile
```

Contile runs, by default, on `http://localhost:8000/`. However, it may be necessary to run the tests
outside of docker, in order to debug functions or manually verify expected results.

**Fetching Tiles**

Contile will attempt to look up your IP address if you access it using Firefox. The easiest way to
get a test response back would be to craft a curl request. For example (presuming that Contile is
running on `http://localhost:8000/v1/tiles`):

```sh
curl -v \
    -H "User-Agent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:10.0) Gecko/20100101 Firefox/91.0'" \
    -H "X-Forwarded-For: '89.160.20.115'" \
    "http://localhost:8000/v1/tiles"
```

### partner

See the `Local Execution` and `Debugging` sections of the partner [README][5]

## Maintenance

The contract test maintenance schedule cadence is once a quarter and should include
updating the following:

1. [poetry][6] version and python dependencies
    * [ ] [pyproject.toml][13]
    * [ ] [poetry.lock][14]
2. [Docker][15] artifacts
    * [ ] client [Dockerfile][16]
    * [ ] partner [Dockerfile][17]
    * [ ] [docker-compose.yml][18]
    * [ ] [docker-compose.204.yml][19]
    * [ ] [docker-compose.init_error.yml][20]
    * [ ] [docker-compose.tiles_cache.yml][21]
3. [CircleCI][22] contract test jobs
    * [ ] [config.yml][23]
4. Documentation
    * [ ] client [README][4]
    * [ ] partner [README][5]
    * [ ] contract tests [README][24]

[1]: https://github.com/mozilla-services/contile-integration-tests
[2]: sequence_diagram.png
[3]: https://miro.com/app/board/uXjVOkw1f-s=/
[4]: ./client/README.md
[5]: ./partner/README.md
[6]: https://python-poetry.org/docs/#installation
[7]: https://github.com/pyenv/pyenv#installation
[8]: https://github.com/pyenv/pyenv-virtualenv#installation
[9]: https://pycqa.github.io/isort/
[10]: https://black.readthedocs.io/en/stable/
[11]: https://flake8.pycqa.org/en/latest/
[12]: https://mypy-lang.org/
[13]: ./pyproject.toml
[14]: ./poetry.lock
[15]: https://docs.docker.com/
[16]: ./client/Dockerfile
[17]: ./partner/Dockerfile
[18]: ./docker-compose.yml
[19]: ./docker-compose.204.yml
[20]: ./docker-compose.init_error.yml
[21]: ./docker-compose.yml
[22]: https://circleci.com/docs/
[23]: /.circleci/config.yml
[24]: ./README.md
[25]: ./volumes/client
[26]: ./volumes/contile
[27]: ./volumes/partner
