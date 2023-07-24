# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.


from typing import Callable

import pytest
import requests
from client_models import Service, Step
from exceptions import PartnerRecordsNotClearedError
from requests import Response as RequestsResponse
from requests.adapters import HTTPAdapter, Retry


@pytest.fixture(name="hosts", scope="session")
def fixture_hosts(request) -> dict[Service, str]:
    """Return a dict mapping from a service name to a host name."""

    return {
        Service.CONTILE: request.config.option.contile_url,
        Service.PARTNER: request.config.option.partner_url,
    }


@pytest.fixture(scope="session", name="connect_to_partner")
def fixture_connect_to_partner(hosts: dict[Service, str]) -> Callable[[], None]:
    """Send a request to Partner API root."""

    partner_host: str = hosts[Service.PARTNER]

    def connect_to_partner():
        with requests.Session() as session:
            # Delay Formula = backoff_factor * (2 ^ (total - 1))
            max_retries: Retry = Retry(total=5, backoff_factor=0.1)
            session.mount(f"{partner_host}", HTTPAdapter(max_retries=max_retries))
            response: RequestsResponse = session.get(f"{partner_host}/")
            response.raise_for_status()

    return connect_to_partner


@pytest.fixture(scope="session", autouse=True)
def fixture_session_setup(connect_to_partner: Callable[[], None]):
    """Execute instructions before the test session."""

    connect_to_partner()  # make sure we can connect to the Contile partner

    yield  # Allow tests to execute


@pytest.fixture(name="clear_partner_records")
def fixture_clear_partner_records(hosts: dict[Service, str]) -> Callable[[], None]:
    """Clear Contile request history on partner."""

    partner_host: str = hosts[Service.PARTNER]

    def clear_partner_records():
        response: RequestsResponse = requests.delete(f"{partner_host}/records/")

        if response.status_code != 204:
            raise PartnerRecordsNotClearedError(response)

    return clear_partner_records


@pytest.fixture(scope="function", autouse=True)
def fixture_function_teardown(clear_partner_records: Callable[[], None]):
    """Execute instructions after each test."""

    yield  # Allow test to execute

    clear_partner_records()


def test_contile(hosts: dict[Service, str], steps: list[Step]):
    """Test for requesting tiles from Contile."""

    for step in steps:
        # Each step in a test scenario consists of a request and a response.
        # Use the parameters to perform the request and verify the response.

        method: str = step.request.method
        url: str = f"{hosts[step.request.service]}{step.request.path}"
        headers: dict[str, str] = {
            header.name: header.value for header in step.request.headers
        }

        response: RequestsResponse = requests.request(method, url, headers=headers)

        error_message: str = (
            f"Expected status code {step.response.status_code},\n"
            f"but the status code in the response from {step.request.service.name} is "
            f"{response.status_code}.\n"
            f"The response content is '{response.text}'."
        )

        assert response.status_code == step.response.status_code, error_message

        if response.status_code == 200:
            # If the response status code is 200 OK, load the response content
            # into a Python dict and generate a dict from the response model
            assert response.json() == step.response.content.model_dump(
                exclude_unset=True
            )
            continue

        if response.status_code == 204:
            # If the response status code is 204 No Content, load the response content
            # as text and compare against the value in the response model. This
            # should be an empty string.
            assert response.text == step.response.content
            continue

        # If the request to Contile was not successful, load the response
        # content into a Python dict and compare against the value in the
        # response model, which is expected to be the Contile error code.
        assert response.json() == step.response.content
