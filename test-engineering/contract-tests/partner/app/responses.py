# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import dataclasses
import functools
import logging
from pathlib import Path
from typing import Dict

import yaml
from models import ResponseFromFile

logger = logging.getLogger("partner")


# Define type aliases for the functions in this module
FormFactor = str
OSFamily = str
ResponsesFromFile = Dict[FormFactor, Dict[OSFamily, ResponseFromFile]]


@dataclasses.dataclass(eq=True, frozen=True)
class LoaderConfig:
    """Configuration options for the responses.yml file loader."""

    responses_dir: Path
    default_filename: str = "responses"


def load_responses_from_file(*, config: LoaderConfig, file: Path) -> ResponsesFromFile:
    """Load responses from the given YAML file."""

    logger.debug("load responses from %s", file.relative_to(config.responses_dir))

    with file.open() as f:
        responses_yml = yaml.safe_load(f)

    return {
        form_factor: {
            os_family: ResponseFromFile(**response)
            for os_family, response in os_families.items()
        }
        for form_factor, os_families in responses_yml.items()
    }


@functools.lru_cache(maxsize=None)
def load_responses(
    *, config: LoaderConfig, country_code: str, region_code: str
) -> ResponsesFromFile:
    """Load responses for the given country_code and region_code combination."""

    logger.debug("load responses using config %s", config)
    logger.debug(
        "load responses for country_code '%s' and region_code '%s'",
        country_code,
        region_code,
    )

    country_dir = config.responses_dir / country_code

    if region_code:
        # The region_code value is not an empty string, for example: "NY"
        responses_file = country_dir / f"{region_code}.yml"

        # If there's a responses file for the given country_code and region_code
        # combination load the responses from that file. Do not catch
        # exceptions, because we need those to bubble up.
        if responses_file.exists():
            return load_responses_from_file(config=config, file=responses_file)

    # If the region_code is an empty string or there's no responses file for the
    # given region_code, load the default responses file for the country
    responses_file = country_dir / f"{config.default_filename}.yml"

    if responses_file.exists():
        # Load default responses for the given country, if the file exists
        return load_responses_from_file(config=config, file=responses_file)

    # Load default responses
    return load_responses_from_file(
        config=config, file=config.responses_dir / f"{config.default_filename}.yml"
    )
