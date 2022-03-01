# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import dataclasses
import os
import pathlib
import subprocess

import pytest
import requests
from urllib3.util.retry import Retry

# This should be the parent directory of `tests/`
CWD = pathlib.Path.cwd()


@dataclasses.dataclass
class CloudFunction:
    """Hold information for a Google Cloud Function."""

    region: str
    target: str
    cwd: pathlib.Path
    port: int = dataclasses.field(repr=False)
    protocol: str = dataclasses.field(repr=False, default="http")
    host: str = dataclasses.field(repr=False, default="localhost")
    url: str = dataclasses.field(init=False)

    def __post_init__(self):
        """Create the value for the url field dynamically."""
        self.url = f"{self.protocol}://{self.host}:{self.port}"


@pytest.fixture(name="client_us", scope="session")
def fixture_client_us() -> CloudFunction:
    """Return a Function instance for a client in the US region."""

    return CloudFunction(
        region="US", target="run_geo_smoke_test", cwd=CWD / "client", port=8844
    )


@pytest.fixture(name="client_gb", scope="session")
def fixture_client_gb() -> CloudFunction:
    """Return a Function instance for a client in the GB region."""

    return CloudFunction(
        region="GB", target="run_geo_smoke_test", cwd=CWD / "client", port=8845
    )


@pytest.fixture(name="client_ch", scope="session")
def fixture_client_ch() -> CloudFunction:
    """Return a Function instance for a client in the CH region."""

    return CloudFunction(
        region="CH", target="run_geo_smoke_test", cwd=CWD / "client", port=8846
    )


@pytest.fixture(name="runner", scope="session")
def fixture_runner() -> CloudFunction:
    """Return a Function instance for the runner."""
    return CloudFunction(
        region="US", target="run_geo_smoke_tests", cwd=CWD / "runner", port=8847
    )


@pytest.fixture(name="client_functions", scope="session", autouse=True)
def fixture_client_functions(
    client_us: CloudFunction,
    client_gb: CloudFunction,
    client_ch: CloudFunction,
):
    """Start and terminate new subprocesses for the functions."""

    # See https://cloud.google.com/functions/docs/testing/test-http#integration_tests
    p_clients = [
        subprocess.Popen(
            [
                "functions-framework",
                "--target",
                client.target,
                "--port",
                str(client.port),
            ],
            cwd=str(client.cwd),
            stdout=subprocess.PIPE,
        )
        for client in (client_us, client_gb, client_ch)
    ]

    yield

    # Stop the functions framework process
    for process in p_clients:
        process.kill()
        process.wait()


@pytest.fixture(name="runner_function", scope="session", autouse=True)
def fixture_runner_function(
    client_us: CloudFunction,
    client_gb: CloudFunction,
    client_ch: CloudFunction,
    runner: CloudFunction,
):
    """Start and terminate new subprocesses for the functions."""

    # See https://cloud.google.com/functions/docs/testing/test-http#integration_tests
    p_runner = subprocess.Popen(
        [
            "functions-framework",
            "--target",
            runner.target,
            "--port",
            str(runner.port),
        ],
        cwd=str(runner.cwd),
        stdout=subprocess.PIPE,
        env={
            "CLIENT_URL_US": client_us.url,
            "CLIENT_URL_GB": client_gb.url,
            "CLIENT_URL_CH": client_ch.url,
            **os.environ,
        },
    )

    yield

    # Stop the functions framework process
    p_runner.kill()
    p_runner.wait()


def test_client(client_us: CloudFunction):
    """Trigger a client CloudFunction and check the response."""

    retry_adapter = requests.adapters.HTTPAdapter(
        max_retries=Retry(total=6, backoff_factor=1)
    )

    session = requests.Session()
    session.mount(client_us.url, retry_adapter)

    response = session.post(
        client_us.url,
        json={
            "environment": "STAGE",
            "expected_country": "US",
            "expected_region": "OR",
        },
    )
    assert response.status_code == 200, response.text

    response_data = response.json()
    assert response_data["error"] is None


def test_functions(runner: CloudFunction):
    """Trigger the runner CloudFunction and check the response."""

    retry_adapter = requests.adapters.HTTPAdapter(
        max_retries=Retry(total=6, backoff_factor=1)
    )

    session = requests.Session()
    session.mount(runner.url, retry_adapter)

    response = session.post(
        runner.url,
        json={"environments": ["STAGE", "PROD"]},
    )
    assert response.status_code == 200, response.text

    response_data = response.json()
    assert response_data["results"] == []
