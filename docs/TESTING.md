# Testing and debugging Contile using the contract tests

The contract testing system is optimized to run within a set of related docker images. All information and documentation is stored in `../test-engineering`.

However, it may be necessary to run the tests outside of docker so that you can debug functions or manually verify expected results.

## Running the mock `partner` app

This is a simple response application that returns candidate tiles

### Starting the partner app

`docker-compose run -p 5000:5000 contract-tests/partner`

The mock partner runs, by default, on `http://localhost:5000/`.

The test URI path is `tilesp/desktop` for desktop tiles, or `tilesp/mobile` for mobile tiles.

The following query arguments are required. Optional, undefined elements should be left empty (e.g. `...&dma-code=&...`) Failure to include them will return an 400 error with the missing variables listed in the response (NOTE: This will produce an unexpected 500 or 502 error in Contile.)

* partner - _string_
* sub1 - _string_
* sub2 - _alphanumeric_
* country-code - _2 CAPTIAL LETTER Alpha_
* region-code - _1 to 3 CAPITAL LETTER AlphaNumeric_
* dma-code - _Optional Numeric_
* form-factor - _See `ACCEPTED_{MOBILE,DESKTOP}_FORM_FACTORS`_
* v = `1.0`
* out = `json`
* results = _number of tiles to return, usually 2_

### <a name="no_doc"></a>Running outside of docker

It is possible to run the mock partner app outside of docker. It is ___STRONGLY___ suggested that you run this within it's own Python virtualenv, and possibly it's own shell to prevent environment variable cross contamination.

The `services: partner` block of `contract-tests/docker-compose.yml` list the `environment` and `volumes` needed. The following environment variables are used by the mock partner app.

* PORT - _default port number_
* RESPONSES_DIR - _directory to read the [Tile Values](#tile_values)_
* ACCEPTED_MOBILE_FORM_FACTORS - _list of allowed `form-factors` for `tilesp/mobile` responses_
* ACCEPTED_DESKTOP_FORM_FACTORS - _list of allowed `form-factors` for `tilesp/desktop` responses_

Start the mock partner app from inside the mock partner virtualenv using

```sh
gunicorn -c config/gunicorn_conf.py --preload -k uvicorn.workers.UvicornWorker main:app
```

## Environment Variables

Use the following environment variables for Contile to contact the mock partner server:

```sh
CONTILE_MAXMINDDB_LOC=${ProjectRoot}/mmdb/GeoLite2-City-Test.mmdb
CONTILE_ADM_ENDPOINT_URL=http://localhost:5000/tilesp/desktop
CONTILE_ADM_MOBILE_ENDPOINT_URL=http://localhost:5000/tilesp/mobile
CONTILE_ADM_QUERY_TILE_COUNT=5
CONTILE_ADM_SUB1=sub1_test
CONTILE_ADM_PARTNER_ID=partner_id_test
CONTILE_ADM_HAS_LEGACY_IMAGE='["Example ORG", "Example COM"]'
```

`CONTILE_ADM_TIMEOUT` determines how long to wait for a response from the partner server. The default value is `5` seconds. You may wish to make this much longer if you're debugging.

These would be in addition to any other settings you wish to use for the Contile server.

`http://localhost:5000/tilesp/desktop` and `http://localhost:5000/tilesp/mobile`

## <a name="tile_values"></a>Tile Values

The returned tile values are stored in `contract-tests/volumes/partner/${country-code}/${region-code}.yml`.

If different values are desired, you can either alter these files or you can copy them into a new directory and use the `RESPONSES_DIR` environment variable for the [mock partner app](#no-dock).

## Fetching tiles from Contile

Contile will attempt to look up your IP address if you access it using Firefox. The easiest way to get a test response back would be to craft a curl request. For example (presuming that Contile is running on `http://localhost:8000/v1/tiles`):

```sh
curl -v \
    -H "UserAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:10.0) Gecko/20100101 Firefox/91.0'" \
    -H "X-Forwarded-For: '89.160.20.115'" \
    "http://localhost:8000/v1/tiles"
```
