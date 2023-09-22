# Contract Test Client

This documentaition describes the Python-based HTTP client test framework for the contract tests.

The HTTP client used in the framework supports:

* Requests for tiles from the MTS, with response checks
* Requests for the history of requests from the MTS to the partner API with response checks

The framework implements response models for the MTS and partner APIs.

For more details on contract test design, refer to the Contile Contract Tests [documentation][1].

## Overview

The client is instructed on request and response check actions via scenarios, recorded in the
`scenarios.yml` file. A scenario is defined by a name, a description, and steps.

### Steps

#### Contile Service

* To direct requests to the MTS service, set the `service` value of `request` to `contile`
* The expected content for a `200 OK` response is a collection of tiles

Example:

```yaml
- request:
    service: contile
    method: GET
    path: '/v1/tiles'
    headers:
      - name: User-Agent
        value: 'Mozilla/5.0 (Windows NT 10.0; rv:10.0) Gecko/20100101 Firefox/91.0'
  response:
    status_code: 200
    content:
      tiles:
        - id: 12345
          name: 'Example COM'
          click_url: 'https://example.com/desktop_windows?version=16.0.0&key=22.1&ci=6.2&ctag=1612376952400200000'
          image_url: 'https://example.com/desktop_windows01.jpg'
          image_size: null
          impression_url: 'https://example.com/desktop_windows?id=0001'
          url: 'https://www.example.com/desktop_windows'
        - id: 56789
          name: 'Example ORG'
          click_url: 'https://example.org/desktop_windows?version=16.0.0&key=7.2&ci=8.9&ctag=E1DE38C8972D0281F5556659A'
          image_url: 'https://example.org/desktop_windows02.jpg'
          image_size: null
          impression_url: 'https://example.org/desktop_windows?id=0002'
          url: 'https://www.example.org/desktop_windows'
```

#### Partner Service

* To direct requests to the partner service, set the `service` value of `request` to `partner`
* The expected content for a `200 OK` response is a collection of records
    * Each `record` represents a distinct request made by the MTS to the partner
    * The frequency of a request is denoted by the `count`
* Request history is cleared between scenarios

Example:

```yaml
- request:
    service: partner
    method: GET
    path: '/records/'
    headers:
      - name: 'accept'
        value: '*/*'
  response:
    status_code: 200
    content:
      records:
        - count: 1
          record:
            method: GET
            headers:
              - name: accept
                value: '*/*'
              - name: user-agent
                value: 'contile/1.8.2'
              - name: host
                value: 'partner:5000'
            path: '/tilesp/desktop'
            query_parameters:
              - name: partner
                value: 'partner_id_test'
              - name: sub1
                value: 'sub1_test'
              - name: sub2
                value: 'newtab'
              - name: country-code
                value: 'US'
              - name: region-code
                value: ''
              - name: dma-code
                value: ''
              - name: form-factor
                value: 'desktop'
              - name: os-family
                value: 'windows'
              - name: v
                value: '1.0'
              - name: out
                value: 'json'
              - name: results
                value: '5'
```

## Debugging

To execute the test scenarios outside the client Docker container, expose the Contile and partner
API ports in the docker-compose.yml, set environment variables and use a pytest command. It is
recommended to execute the tests within a Python virtual environment to prevent dependency cross
contamination.

### Environment Setup

This project uses [Poetry][2] for dependency management. For environment setup, it is recommended to
use [pyenv][3] and [pyenv-virtualenv][4], as they work nicely with Poetry.

Project dependencies are listed in the `pyproject.toml` file. To install the dependencies execute:

```shell
poetry install --without partner
```

### Execution

1. Modify `test-engineering/contract/docker-compose.yml`

   In the partner definition, expose port 5000 by adding the following:
    ```yaml
    ports:
      - "5000:5000"
    ```

   In the Contile definition, expose port 8000 by adding the following:
    ```yaml
    ports:
      - "8000:8000"
    ```

2. Run Contile and partner docker containers.

   Execute the following from the project root:
   ```shell
   docker compose -f test-engineering/contract/docker-compose.yml up contile
   ```

3. Run the contract tests

   Execute the following from the project root:
    ```shell
    CONTILE_URL=http://localhost:8000 \
        PARTNER_URL=http://localhost:5000 \
        SCENARIOS_FILE=test-engineering/contract/volumes/client/scenarios.yml \
        pytest test-engineering/contract/client/tests/test_contile.py --vv
    ```
    * Environment variables can alternatively be set in a pytest.ini file or through an IDE
      configuration
    * Tests can be run individually using [-k _expr_][5]

      Example executing the `success_desktop_windows` scenario:
      ```shell
      pytest test-engineering/contract/client/tests/test_contile.py \
          -k success_desktop_windows
      ```

[1]: ./contract-tests.md
[2]: https://python-poetry.org/docs/#installation
[3]: https://github.com/pyenv/pyenv#installation
[4]: https://github.com/pyenv/pyenv-virtualenv#installation
[5]: https://docs.pytest.org/en/latest/example/markers.html#using-k-expr-to-select-tests-based-on-their-name
