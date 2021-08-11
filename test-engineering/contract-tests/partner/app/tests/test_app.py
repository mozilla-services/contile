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
    response = client.get(
        "/tilesp",
        params={
            "partner": "demofeed",
            "sub1": "123456789",
            "sub2": "placement1",
            "country-code": "US",
            "region-code": "NY",
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
        "CA",
        "DE",
        "GB",
        "FR",
        "AU",
        "IT",
        "MX",
        "IN",
        "BR",
        "ES",
    ],
    ids=[
        "Canada",
        "Germany",
        "UK",
        "France",
        "Australia",
        "Italy",
        "Mexico",
        "India",
        "Brazil",
        "Spain",
    ],
)
def test_read_tilesp_accepted_country_code(client, country_code):
    """Test that the API endpoint accepts countries Contile has been rolled out
    to for the country-code query parameter.

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
