# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.


import pathlib
import sys

import pytest
import yaml
from fastapi.testclient import TestClient

from ..main import app


@pytest.fixture(name="version")
def fixture_version() -> str:
    return f"{sys.version_info.major}.{sys.version_info.minor}"


@pytest.fixture(name="client")
def fixture_client() -> TestClient:
    return TestClient(app)


@pytest.fixture(name="available_tiles")
def fixture_available_tiles():
    with pathlib.Path("app/tiles.yml").open() as f:
        available_tiles = yaml.safe_load(f)
    return available_tiles
