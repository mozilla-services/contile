# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.


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
                "name": "Example",
                "click_url": "https://example.com/desktop_macos?version=16.0.0&key=22.1&ci=6.2&ctag=1612376952400200000",
                "image_url": "https://example.com/desktop_macos01.jpg",
                "impression_url": "https://example.com/desktop_macos?id=0001",
                "advertiser_url": "https://www.example.com/desktop_macos",
            },
            {
                "id": 56790,
                "name": "Example",
                "click_url": "https://example.com/desktop_macos?version=16.0.0&key=7.2&ci=8.9&ctag=E1DE38C8972D0281F5556659A",
                "image_url": "https://example.com/desktop_macos02.jpg",
                "impression_url": "https://example.com/desktop_macos?id=0002",
                "advertiser_url": "https://www.example.com/desktop_macos",
            },
        ]
    }
