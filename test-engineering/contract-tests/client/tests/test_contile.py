# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.


from typing import Callable, Dict, List

import pytest
import requests
from requests.models import Response

from models import Step


@pytest.fixture(name="hosts", scope="session")
def fixture_hosts(request) -> Dict[str, str]:
    """Return a dict mapping from a service name to a host name."""

    return {
        "contile": request.config.option.contile_url,
        "partner": request.config.option.partner_url,
    }


@pytest.fixture(name="clear_partner_records")
def fixture_clear_partner_records(hosts) -> Callable:
    """Clear Contile request history on partner."""

    partner_host = hosts["partner"]

    def _clear_records():
        r: Response = requests.request("DELETE", f"{partner_host}/records/")

        error_message: str = (
            f"Request status code was not as expected. Records may not have cleared "
            f"after test run, details: '{r.text}'."
        )

        assert r.status_code == 204, error_message

    return _clear_records


@pytest.fixture(scope="function", autouse=True)
def fixture_function_teardown(clear_partner_records: Callable):
    """Execute instructions after each test."""

    yield  # Allow test to execute

    clear_partner_records()


def test_contile(hosts: Dict[str, str], steps: List[Step]):
    """Test for requesting tiles from Contile."""

    for step in steps:
        # Each step in a test scenario consists of a request and a response.
        # Use the parameters to perform the request and verify the response.

        method: str = step.request.method
        url: str = f"{hosts[step.request.service.value]}{step.request.path}"
        headers: Dict[str, str] = {
            header.name: header.value for header in step.request.headers
        }

        r: Response = requests.request(method, url, headers=headers)

        error_message: str = (
            f"Expected status code {step.response.status_code},\n"
            f"but the status code in the response from Contile is {r.status_code}.\n"
            f"The response content is '{r.text}'."
        )

        assert r.status_code == step.response.status_code, error_message

        if r.status_code == 200:
            # If the response status code is 200 OK, load the response content
            # into a Python dict and generate a dict from the response model
            assert r.json() == step.response.content.dict()
            continue

        if r.status_code == 204:
            # If the response status code is 204 No Content, load the response content
            # as text and compare against the value in the response model. This
            # should be an empty string.
            assert r.text == step.response.content
            continue

        # If the request to Contile was not successful, load the response
        # content into a Python dict and compare against the value in the
        # response model, which is expected to be the Contile error code.
        assert r.json() == step.response.content
