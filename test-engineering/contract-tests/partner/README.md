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

## Running the service

You can run the service using `docker compose` from the root directory:

```text
docker compose run -p 5000:5000 partner
```

## Tiles API

Example request:

```text
curl \
  -X 'GET' \
  -H 'accept: application/json' \
  'http://0.0.0.0:5000/tilesp?partner=demofeed&sub1=123456789&sub2=placement1&country-code=US&region-code=NY&form-factor=desktop&os-family=macos&v=1.0&out=json&results=2'
```

Example response body:

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
