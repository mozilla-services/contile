# Integration test for Contile

This directory contains a simple integration test for the Contile server.

## Installation

1) First, create a virtualenv installation of python
`python -m venv venv`

This will create a local install of the python. You can then "activate" it by calling
`sh venv/bin/activate.sh`

After activation, you no longer need to specify the path: `venv/bin/`

2) Install requirements.txt
`pip -r requirements.txt`

## Running the test

There are several test related environment variables that may be specified:

| var | description|
|--|--|
| **CONTILE_TEST_URL** | HTTP URI to the test server. Defaults to `http://localhost:8000` |
| **CONTILE_TEST_SERVER** | Path to the locally compiled Contile server executable. Defaults to `../../target/debug/contile` |
| **CONTILE_TEST_NOSERVER** | Do not attempt to start up the locally compiled Contile server executable |

You can run the tests by running
```pytest . ```

You can specify `pytest -sx . ` if you want tests to stop at the first failure.

The test will attempt to start the locally compiled Contile server (unless `CONTILE_TEST_NOSERVER` is set) and run the local tests checking for returned values.

## Crafting tests

Tests are included in the `TestAdm` class. Please note that returned values from a live server may differ significantly from the test data, so examining the return results may need be blocked by checking if `settings.get("noserver")` is not set.

The server is started with the `CONTILE_TEST_MODE` flag set. This will prevent the server from using the remote ADM server and instead pull data from a test directory `./test_data`. This contains JSON formatted files. Names must contain only alphanumeric and `_`. Also note that the test server will use the `../../adm_settings_test.json` configuration file. Be sure that your test data responses meets the criteria specified in the `adm_settings_test.json` file.

Tests can specify the data that can be returned by the `adm` component by including a `Fake-Response` header, which contains only the file name of the test_data file. (e.g. to include `./test_data/bad_adv.json` as the adm response, use `Fake-Response: bad_adv`)