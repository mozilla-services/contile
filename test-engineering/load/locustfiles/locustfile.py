# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

"""Load test module."""

import os
import random

from locust import FastHttpUser, between, task

from common.location import parse_subdivision_codes_file
from common.user_agent import FIREFOX_USER_AGENTS, NON_FIREFOX_USER_AGENTS

TILES_API: str = "/v1/tiles"

# Environment Variables
CONTILE_LOCATION_TEST_HEADER: str = os.environ.get(
    "CONTILE_LOCATION_TEST_HEADER", "X-Test-Location"
)

# Global Test Data Setup
CLDR_SUBDIVISION_FILE_PATH: str = "data/unicode_cldr_subdivision_codes.xml"
locations: list[str] = parse_subdivision_codes_file(CLDR_SUBDIVISION_FILE_PATH)


class ContileFirefoxUser(FastHttpUser):
    """User that sends requests to the Contile API from a Firefox User-Agent."""

    weight = 75  # Override probability of user class being chosen (out of 100)

    @task
    def get_tiles(self) -> None:
        """Request tiles from Contile."""
        user_agent: str = random.choice(FIREFOX_USER_AGENTS)
        location: str = random.choice(locations)
        headers: dict[str, str] = {
            "User-Agent": user_agent,
            CONTILE_LOCATION_TEST_HEADER: location,
        }
        # The `name` argument is specified to use as the label in Locust’s statistics
        # instead of the URL path.
        name: str = f"{TILES_API} User-Agent: {user_agent} Location: {location}"

        with self.client.get(
            url=TILES_API, headers=headers, catch_response=True, name=name
        ) as response:
            if response.status_code not in (200, 204, 304):
                response.failure(
                    f"{response.status_code=}, expected 200,204,304 {response.text=}"
                )
            if response.status_code == 0:
                # Do not classify as failure
                # The HttpSession catches any requests.RequestException thrown by
                # Session (caused by connection errors, timeouts or similar),
                # instead returning a dummy Response object with status_code set to 0
                # and content set to None.
                response.success()


class ContileNonFirefoxUser(FastHttpUser):
    """User that sends requests to the Contile API from a non-Firefox User-Agent."""

    wait_time = between(5, 10)  # Override the time between the execution of tasks
    weight = 25  # Override the probability of user class being chosen (out of 100)

    @task
    def get_tiles(self) -> None:
        """Request tiles from Contile."""
        user_agent: str = random.choice(NON_FIREFOX_USER_AGENTS)
        location: str = random.choice(locations)
        headers: dict[str, str] = {
            "User-Agent": user_agent,
            CONTILE_LOCATION_TEST_HEADER: location,
        }
        # The `name` argument is specified to use as the label in Locust’s statistics
        # instead of the URL path.
        name: str = f"{TILES_API} User-Agent: {user_agent} Location: {location}"

        with self.client.get(
            url=TILES_API, headers=headers, catch_response=True, name=name
        ) as response:
            # Contile should send an empty response to a request from a non-Firefox
            # user-agent
            if response.status_code == 403:
                response.success()
            elif response.status_code == 0:
                # Do not classify as failure
                # The HttpSession catches any requests.RequestException thrown by
                # Session (caused by connection errors, timeouts or similar),
                # instead returning a dummy Response object with status_code set to 0
                # and content set to None.
                response.success()
            else:
                response.failure(
                    f"{response.status_code=}, expected 403, {response.text=}"
                )
