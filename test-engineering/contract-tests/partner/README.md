# partner

This directory contains a Python-based web service. The HTTP API of this service
implements the API specification of the partner API that MTS connects to when
requesting tiles to pass along to Firefox for display.

## Setup

Install all requirements via [pip-tools][pip-tools]:

```text
pip-sync requirements.txt dev-requirements.txt
```

## Code checks and tests

With requirements installed run the code checks and test via [tox][tox]:

```text
tox
```

See the tox configuration in the `tox.ini` for the list of environments this
will run.

## Local Execution and Debugging

To run the service locally, execute the following from the contract-tests root:

```text
docker compose run -p 5000:5000 partner
```

The mock partner runs, by default, on `http://localhost:5000/`.

The test URI path is `tilesp/desktop` for desktop tiles, or `tilesp/mobile` for mobile 
tiles.

The following query arguments are required. Optional, undefined elements should be left 
empty (e.g. `...&dma-code=&...`) Failure to include them will return a 400 error with 
the missing variables listed in the response (NOTE: This will produce an unexpected 500 
or 502 error in Contile.)

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

It is possible to run the mock partner app outside of docker. It is ___STRONGLY___ 
suggested that you run this within its own Python virtualenv, and possibly its own 
shell to prevent environment variable cross contamination.

The `services: partner` block of `contract-tests/docker-compose.yml` lists the 
`environment` and `volumes` needed. The following environment variables are used by 
the mock partner app.

* PORT - _default port number_
* RESPONSES_DIR - _directory to read the [Tile Values](#tile_values)_
* ACCEPTED_MOBILE_FORM_FACTORS - _list of allowed `form-factors` for `tilesp/mobile` responses_
* ACCEPTED_DESKTOP_FORM_FACTORS - _list of allowed `form-factors` for `tilesp/desktop` responses_

Start the mock partner app from inside the mock partner virtualenv using

```sh
gunicorn -c config/gunicorn_conf.py --preload -k uvicorn.workers.UvicornWorker main:app
````

### Environment Variables

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

`CONTILE_ADM_TIMEOUT` determines how long to wait for a response from the partner server. 
The default value is `5` seconds. You may wish to make this much longer if you're debugging.

These would be in addition to any other settings you wish to use for the Contile server.

`http://localhost:5000/tilesp/desktop` and `http://localhost:5000/tilesp/mobile`

### <a name="tile_values"></a>Tile Values

The returned tile values are stored in 
`contract-tests/volumes/partner/${country-code}/${region-code}.yml`.

If different values are desired, you can either alter these files or you can copy them 
into a new directory and use the `RESPONSES_DIR` environment variable for the 
[mock partner app](#no-dock).

## API

Once the API service is running, API documentation can be found at 
`http://0.0.0.0:5000/docs`.

### Records

**GET**: Endpoint to retrieve all historical Contile request records with a counter.

Example: 

Request

```text
curl \
  -X 'GET' \
  -H 'accept: application/json' \
  'http://0.0.0.0:5000/records/'
```

Response:

Code: `200`

Body:
```json
{
  "records": [
    {
      "count": 1,
      "record": {
        "method": "GET",
        "headers": [
          {
            "name": "host",
            "value": "0.0.0.0:5000"
          },
          {
            "name": "user-agent",
            "value": "curl/7.79.1"
          },
          {
            "name": "accept",
            "value": "application/json"
          }
        ],
        "path": "/tilesp/desktop",
        "query_parameters": [
          {
            "name": "partner",
            "value": "demofeed"
          },
          {
            "name": "sub1",
            "value": "123456789"
          },
          {
            "name": "sub2",
            "value": "placement1"
          },
          {
            "name": "country-code",
            "value": "US"
          },
          {
            "name": "region-code",
            "value": "NY"
          },
          {
            "name": "dma-code",
            "value": "532"
          },
          {
            "name": "form-factor",
            "value": "desktop"
          },
          {
            "name": "os-family",
            "value": "macos"
          },
          {
            "name": "v",
            "value": "1.0"
          },
          {
            "name": "out",
            "value": "json"
          },
          {
            "name": "results",
            "value": "2"
          }
        ]
      }
    }
  ]
}
```

**DELETE**: Endpoint to delete all historical Contile request records.

Example:

Request

```text
curl \
  -X 'DELETE' \
  -H 'accept: */*' \
  'http://0.0.0.0:5000/records/'
```

Response

Code: `204`

Body: `N/A`

### Tiles

**GET**: Endpoint for requests from Contile.

Example:

Request

```text
curl \
  -X 'GET' \
  -H 'accept: application/json' \
  'http://0.0.0.0:5000/tilesp/desktop?partner=demofeed&sub1=123456789&sub2=placement1&country-code=US&region-code=NY&dma-code=532&form-factor=desktop&os-family=macos&v=1.0&out=json&results=2'
```

Response

Code: `200`

Body:

```json
{
  "tiles": [
    {
      "id": 12346,
      "name": "Example COM",
      "click_url": "https://example.com/desktop_macos?version=16.0.0",
      "image_url": "https://example.com/desktop_macos01.jpg",
      "impression_url": "https://example.com/desktop_macos?id=0001",
      "advertiser_url": "https://www.example.com/desktop_macos"
    },
    {
      "id": 56790,
      "name": "Example ORG",
      "click_url": "https://example.org/desktop_macos?version=16.0.0",
      "image_url": "https://example.org/desktop_macos02.jpg",
      "impression_url": "https://example.org/desktop_macos?id=0002",
      "advertiser_url": "https://www.example.org/desktop_macos"
    }
  ]
}
```

[tox]: https://pypi.org/project/tox/
[pip-tools]: https://pypi.org/project/pip-tools/
