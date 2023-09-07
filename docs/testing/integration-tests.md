# Integration Tests for Contile

This documentation describes the integration tests for the Contile server.

## Installation

1. First, create a virtualenv installation of python
`python -m venv venv`
   or use python3 for Python 3 or higher version
`python3 -m venv venv`

This will create a local install of the python. You can then "activate" it by calling
`source venv/bin/activate` (note, refer to the [python virtualenv](https://docs.python.org/3/library/venv.html)
documentation for system specific details.)

After activation, you no longer need to specify the path: `venv/bin/`

2. Install requirements.txt in the Test dir
`pip install -r requirements.txt`

## Running the test

There are several integration test related environment variables that may be specified:

| var | description |
|--|--|
| **CONTILE_TEST_URL** | HTTP URI to the test server. Defaults to `http://localhost:8000` |
| **CONTILE_TEST_SERVER** | Path to the locally compiled Contile server executable. Defaults to `contile/target/debug/contile` |
| **CONTILE_TEST_NOSERVER** | Do not attempt to start up the locally compiled Contile server executable |

These are different than the Contile test arguments:

| var | description |
|--|--|
| **CONTILE_TEST_MODE** | Places the server in "Test Mode". There are several possible test modes available: `TestFakeResponse`: which will cause it NOT to call out to the ADM server, but use test response files. `TestTimeout` which will emulate a timeout error when trying to fetch a tile from ADM server. |
| **CONTILE_TEST_FILE_PATH** | The path to the ADM fake response files. |
| **CONTILE_ADM_SETTINGS** | The path to the ADM settings to be used for this run |

The tests will provide their own ENV var values for these unless specified as part of the exec command.
(e.g. ```CONTILE_TEST_FILE_PATH="/tmp/test_data" pytest .```)

You can run the tests by running
```pytest . ```

You can specify `pytest -sx . ` if you want tests to stop at the first failure.

The test will attempt to start the locally compiled Contile server (unless `CONTILE_TEST_NOSERVER` is set) and run the local tests checking for returned values.

## Crafting Tests

Tests are included in the `TestAdm` class. Please note that returned values from a live server may differ significantly from the test data, so examining the return results may need to be blocked by checking if `settings.get("noserver")` is not set.

The server is started with the `CONTILE_TEST_MODE` flag set. This will prevent the server from using the remote ADM server and instead pull data from a test directory `./test_data`. This contains JSON formatted files. Names must be lower case, contain only alphanumeric and `_`, and be properly formatted JSON. The application presumes that these files are located in the relative directory of `./tools/test/test_data`, however that presumes that you are running in the Project Root directory (The same directory that contains the `Cargo.toml` file). If this
is not the case, or the test files are in a different path, be sure to update the `CONTILE_TEST_FILE_PATH` variable to point to the correct
directory. (e.g. if `CONTILE_TEST_MODE` is set to `/tmp/test_data`, then test an ADM data `DEFAULT` response for a request with no `Fake-Header` would be stored as `/tmp/test_data/default.json`)

Also note that the test server will use the `../../adm_settings_test.json` configuration file. Be sure that your test data responses meets the criteria specified in the `adm_settings_test.json` file. Like `CONTILE_TEST_FILE_PATH` if the path or file name is different, be sure to specify the correct value with `CONTILE_ADM_SETTINGS`.

Tests can specify the data that can be returned by the `adm` component by including a `Fake-Response` header, which contains only the file name of the test_data file. (e.g. to include `./test_data/bad_adv.json` as the adm response, use `Fake-Response: bad_adv`)