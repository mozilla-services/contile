# partner

Mock service for the API of the partner.

## Setup

Install all requirements via [pip-tools][pip-tools]:

```text
pip-sync requirements.txt dev-requirements.txt
```

[pip-tools]: https://pypi.org/project/pip-tools/

## Running the service

Build the Docker image:

```text
docker build -t partner .
```

Run the container:

```text
docker run --rm -p 8000:8000 partner
```

## Tiles API

Example request:

```text
curl -X 'GET' \
  'http://0.0.0.0:8000/tilesp?partner=demofeed&sub1=123456789&sub2=placement1&country-code=US&region-code=NY&form-factor=desktop&os-family=macOS&v=1.0&out=json&results=2' \
  -H 'accept: application/json'
```

Example response body:

```json
[
  {
    "id": 12345,
    "name": "tile 12345",
    "click_url": "example click_url",
    "image_url": "example image_url",
    "impression_url": "example impression_url",
    "advertiser_url": "example advertiser_url"
  },
  {
    "id": 56789,
    "name": "tile 56789",
    "click_url": "example click_url",
    "image_url": "example image_url",
    "impression_url": "example impression_url",
    "advertiser_url": "example advertiser_url"
  }
]
```

## Unit tests

You can run the unit tests for the service with pytest:

```text
pytest
```
