# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import os
import pathlib
from typing import Dict, Type, Union

import pytest
import yaml
from client_models import Records, Scenario, Service, Tiles

SERVICE_MODEL = Union[Type[Records], Type[Tiles]]
SERVICE_MODELS: Dict[Service, SERVICE_MODEL] = {
    Service.PARTNER: Records,
    Service.CONTILE: Tiles,
}


def pytest_configure(config):
    """Load test scenarios from file."""

    scenarios_file = os.environ["SCENARIOS_FILE"]

    with pathlib.Path(scenarios_file).open() as f:
        loaded_scenarios = yaml.safe_load(f)

    config.contile_scenarios = [
        Scenario(**scenario) for scenario in loaded_scenarios["scenarios"]
    ]

    # Check that all 200 OK responses in test scenarios contain correct
    # information and FastAPI model instances were created for them.
    for scenario in config.contile_scenarios:
        for i, step in enumerate(scenario.steps):

            if step.response.status_code != 200:
                continue

            expected_model: SERVICE_MODEL = SERVICE_MODELS.get(step.request.service)

            if not isinstance(step.response.content, expected_model):
                raise pytest.UsageError(
                    f"Failed to create {expected_model.__name__} "
                    f"model for '200 OK' response content in "
                    f"step {i} of scenario {scenario.name}"
                )


def pytest_generate_tests(metafunc):
    """Generate tests from the loaded test scenarios."""

    ids = []
    argvalues = []

    for scenario in metafunc.config.contile_scenarios:
        ids.append(scenario.name)
        argvalues.append([scenario.steps])

    metafunc.parametrize(["steps"], argvalues, ids=ids)


def pytest_addoption(parser):
    """Define custom CLI options."""
    contile_group = parser.getgroup("contile")

    contile_group.addoption(
        "--contile-url",
        action="store",
        dest="contile_url",
        help="Contile endpoint URL",
        metavar="CONTILE_URL",
        default=os.environ.get("CONTILE_URL"),
        type=str,
        required=False,
    )

    contile_group.addoption(
        "--partner-url",
        action="store",
        dest="partner_url",
        help="Partner endpoint URL",
        metavar="PARTNER_URL",
        default=os.environ.get("PARTNER_URL"),
        type=str,
        required=False,
    )
