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


def test_read_tilesp(client, available_tiles):
    results = 2
    response = client.get(
        "/tilesp"
        "?partner=demofeed"
        "&sub1=123456789"
        "&sub2=placement1"
        "&country-code=US"
        "&region-code=NY"
        "&form-factor=desktop"
        "&os-family=macOS"
        "&v=1.0"
        f"&results={results}"
    )

    response_body = {"tiles": available_tiles[:results]}

    assert response.status_code == 200
    assert response.json() == response_body
