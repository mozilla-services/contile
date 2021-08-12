# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import pytest


def test_read_root(client, version):
    response = client.get("/")
    message = (
        f"Hello world! From FastAPI running on Uvicorn "
        f"with Gunicorn. Using Python {version}"
    )

    assert response.status_code == 200
    assert response.json() == {"message": message}


def test_read_tilesp(client):
    """Test that /tilesp API endpoint returns the expected response for the
    example values for the required query parameters from the API specification.
    """
    response = client.get(
        "/tilesp",
        params={
            "partner": "demofeed",
            "sub1": "123456789",
            "sub2": "placement1",
            "country-code": "US",
            "region-code": "NY",
            "dma-code": "532",
            "form-factor": "desktop",
            "os-family": "macos",
            "v": "1.0",
            "results": "2",
        },
    )

    assert response.status_code == 200
    assert response.json() == {
        "tiles": [
            {
                "id": 12346,
                "name": "Example COM",
                "click_url": "https://example.com/desktop_macos?version=16.0.0&key=22.1&ci=6.2&ctag=1612376952400200000",
                "image_url": "https://example.com/desktop_macos01.jpg",
                "impression_url": "https://example.com/desktop_macos?id=0001",
                "advertiser_url": "https://www.example.com/desktop_macos",
            },
            {
                "id": 56790,
                "name": "Example ORG",
                "click_url": "https://example.org/desktop_macos?version=16.0.0&key=7.2&ci=8.9&ctag=E1DE38C8972D0281F5556659A",
                "image_url": "https://example.org/desktop_macos02.jpg",
                "impression_url": "https://example.org/desktop_macos?id=0002",
                "advertiser_url": "https://www.example.org/desktop_macos",
            },
        ]
    }


@pytest.mark.parametrize(
    "sub2",
    [
        "invalid-param",
        "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz",
        "🛒📈🤖",
    ],
    ids=["hyphen_in_value", "exceeds_max_characters", "emoji"],
)
def test_read_tilesp_validate_sub2(client, sub2):
    """Test that only alphanumeric characters and maximum 128 characters are
    accepted as values for the sub2 query parameter.

    See https://github.com/mozilla-services/contile-integration-tests/issues/38
    """
    response = client.get(
        "/tilesp",
        params={
            "partner": "demofeed",
            "sub1": "123456789",
            "sub2": sub2,
            "country-code": "US",
            "region-code": "NY",
            "form-factor": "desktop",
            "os-family": "macos",
            "v": "1.0",
            "results": "2",
        },
    )

    assert response.status_code == 400

    response_content = response.json()
    assert "tiles" not in response_content
    assert "status" in response_content
    assert "count" in response_content
    assert "response" in response_content


@pytest.mark.parametrize(
    "country_code",
    [
        "invalid-param",
        "us",
        "11",
        "1U",
        "USAC",
        "🛒📈🤖",
    ],
    ids=[
        "hyphen_in_value",
        "all_lowercase",
        "numeric",
        "alpha_numeric",
        "exceeds_max_characters",
        "emoji",
    ],
)
def test_read_tilesp_validate_country_code(client, country_code):
    """Test that only two uppercase characters are accepted as values for the
    country-code query parameter.

    See https://github.com/mozilla-services/contile-integration-tests/issues/39
    """
    response = client.get(
        "/tilesp",
        params={
            "partner": "demofeed",
            "sub1": "123456789",
            "sub2": "sub2",
            "country-code": country_code,
            "region-code": "NY",
            "form-factor": "desktop",
            "os-family": "macos",
            "v": "1.0",
            "results": "2",
        },
    )

    assert response.status_code == 400

    response_content = response.json()
    assert "tiles" not in response_content
    assert "status" in response_content
    assert "count" in response_content
    assert "response" in response_content


@pytest.mark.parametrize(
    "country_code, region_code",
    [
        ("CA", "BC"),
        ("DE", "BE"),
        ("GB", "SCT"),
        ("FR", "BRE"),
        ("AU", "WA"),
        ("IT", "52"),
        ("MX", "CHH"),
        ("IN", "PB"),
        ("BR", "RJ"),
        ("ES", "M"),
        ("US", ""),
    ],
)
def test_read_tilesp_accepted_country_region_code(client, country_code, region_code):
    """Test that the API endpoint accepts region codes from countries Contile
    has been rolled out to for the region-code query parameter.

    See https://github.com/mozilla-services/contile-integration-tests/issues/39
    See https://github.com/mozilla-services/contile-integration-tests/issues/40
    """
    response = client.get(
        "/tilesp",
        params={
            "partner": "demofeed",
            "sub1": "123456789",
            "sub2": "sub2",
            "country-code": country_code,
            "region-code": region_code,
            "form-factor": "desktop",
            "os-family": "macos",
            "v": "1.0",
            "results": "2",
        },
    )

    assert response.status_code == 200
    assert response.json() == {
        "tiles": [
            {
                "id": 12346,
                "name": "Example COM",
                "click_url": "https://example.com/desktop_macos?version=16.0.0&key=22.1&ci=6.2&ctag=1612376952400200000",
                "image_url": "https://example.com/desktop_macos01.jpg",
                "impression_url": "https://example.com/desktop_macos?id=0001",
                "advertiser_url": "https://www.example.com/desktop_macos",
            },
            {
                "id": 56790,
                "name": "Example ORG",
                "click_url": "https://example.org/desktop_macos?version=16.0.0&key=7.2&ci=8.9&ctag=E1DE38C8972D0281F5556659A",
                "image_url": "https://example.org/desktop_macos02.jpg",
                "impression_url": "https://example.org/desktop_macos?id=0002",
                "advertiser_url": "https://www.example.org/desktop_macos",
            },
        ]
    }


@pytest.mark.parametrize(
    "region_code",
    [
        "US-AZ",
        "12AS",
        "ny",
        "🛒📈🤖",
    ],
    ids=[
        "hyphen_in_value",
        "exceeds_max_characters",
        "all_lower_cases",
        "emoji",
    ],
)
def test_read_tilesp_validate_region_code(client, region_code):
    """Test only alphanumeric characters and maximum 3 characters are accepted
    as values for the region_code query parameter.

    See https://github.com/mozilla-services/contile-integration-tests/issues/40
    """
    response = client.get(
        "/tilesp",
        params={
            "partner": "demofeed",
            "sub1": "123456789",
            "sub2": "sub2",
            "country-code": "US",
            "region-code": region_code,
            "form-factor": "desktop",
            "os-family": "macos",
            "v": "1.0",
            "results": "2",
        },
    )

    assert response.status_code == 400

    response_content = response.json()
    assert "tiles" not in response_content
    assert "status" in response_content
    assert "count" in response_content
    assert "response" in response_content


@pytest.mark.parametrize(
    "dma_code",
    [
        "US-AZ",
        "ny1",
        "🛒📈🤖",
    ],
    ids=[
        "hyphen_in_value",
        "alpha_numeric",
        "emoji",
    ],
)
def test_read_tilesp_validate_dma_code(client, dma_code):
    """Test that if provided the DMA code is submitted as an unsigned integer.

    See https://github.com/mozilla-services/contile-integration-tests/issues/62
    """
    response = client.get(
        "/tilesp",
        params={
            "partner": "demofeed",
            "sub1": "123456789",
            "sub2": "sub2",
            "country-code": "US",
            "region-code": "NY",
            "dma-code": dma_code,
            "form-factor": "desktop",
            "os-family": "macos",
            "v": "1.0",
            "results": "2",
        },
    )

    assert response.status_code == 400

    response_content = response.json()
    assert "tiles" not in response_content
    assert "status" in response_content
    assert "count" in response_content
    assert "response" in response_content

    fastapi_error = {
        "body": None,
        "detail": [
            {
                "ctx": {"pattern": "^([0-9]+)?$"},
                "loc": ["query", "dma-code"],
                "msg": 'string does not match regex "^([0-9]+)?$"',
                "type": "value_error.str.regex",
            }
        ],
    }
    assert response_content["test"] == fastapi_error


def test_read_tilesp_error_for_unknown_query_params(client):
    """Test that the API endpoint returns an error for any parameter other than
    the accepted query parameters.

    See https://github.com/mozilla-services/contile-integration-tests/issues/41
    """
    response = client.get(
        "/tilesp",
        params={
            "partner": "demofeed",
            "sub1": "123456789",
            "sub2": "sub2",
            "country-code": "US",
            "region-code": "NY",
            "form-factor": "desktop",
            "os-family": "macos",
            "v": "1.0",
            "results": "2",
            "unknown-query-param": "unknown-query-param",
        },
    )

    assert response.status_code == 400

    response_content = response.json()
    assert "tiles" not in response_content
    assert "status" in response_content
    assert "count" in response_content
    assert "response" in response_content
