# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.


import logging
import os
from pathlib import Path

import pytest
from responses import LoaderConfig, load_responses


@pytest.fixture(name="responses_dir", autouse=True)
def fixture_cache_clear() -> None:
    """Clear the LRU cache before every test."""
    load_responses.cache_clear()


@pytest.fixture(name="loader_config")
def fixture_loader_config() -> LoaderConfig:
    """Return a Path to the directory containing responses files."""
    return LoaderConfig(responses_dir=Path(os.environ["RESPONSES_DIR"]))


def test_fallback_to_default_for_country(caplog, loader_config: LoaderConfig):
    """Test that load_responses() falls back to loading a country's default
    responses if region_code is an empty string.
    """
    with caplog.at_level(logging.DEBUG, logger="partner"):
        responses = load_responses(
            config=loader_config, country_code="US", region_code=""
        )

    assert caplog.record_tuples == [
        (
            "partner",
            logging.DEBUG,
            f"load responses using config {loader_config}",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses for country_code 'US' and region_code ''",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses from US/responses.yml",
        ),
    ]

    assert responses


def test_fallback_to_global_default(caplog, loader_config: LoaderConfig):
    """Test that load_responses() falls back to loading the global default
    responses if it cannot find a responses.yml for the given country_code and
    region_code.
    """
    with caplog.at_level(logging.DEBUG, logger="partner"):
        responses = load_responses(
            config=loader_config, country_code="GB", region_code=""
        )

    assert caplog.record_tuples == [
        (
            "partner",
            logging.DEBUG,
            f"load responses using config {loader_config}",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses for country_code 'GB' and region_code ''",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses from responses.yml",
        ),
    ]

    assert responses


def test_lru_cache(caplog, loader_config: LoaderConfig):
    """Test that the load_responses method is cached and responses.yml files are
    only loaded once for a given country_code and region_code combination.
    """

    with caplog.at_level(logging.DEBUG, logger="partner"):
        load_responses(config=loader_config, country_code="US", region_code="NY")
        load_responses(config=loader_config, country_code="DE", region_code="BE")

        # The next call to load_responses is expected to be cached
        load_responses(config=loader_config, country_code="US", region_code="NY")

        load_responses(config=loader_config, country_code="GB", region_code="SCT")

    assert caplog.record_tuples == [
        (
            "partner",
            logging.DEBUG,
            f"load responses using config {loader_config}",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses for country_code 'US' and region_code 'NY'",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses from US/NY.yml",
        ),
        (
            "partner",
            logging.DEBUG,
            f"load responses using config {loader_config}",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses for country_code 'DE' and region_code 'BE'",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses from DE/responses.yml",
        ),
        (
            "partner",
            logging.DEBUG,
            f"load responses using config {loader_config}",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses for country_code 'GB' and region_code 'SCT'",
        ),
        (
            "partner",
            logging.DEBUG,
            "load responses from responses.yml",
        ),
    ]
