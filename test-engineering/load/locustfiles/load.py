# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

"""Load test shape module."""

from typing import Type

from locust import LoadTestShape, User
from locustfile import ContileFirefoxUser, ContileNonFirefoxUser
from pydantic import BaseModel

TickTuple = tuple[int, float, list[Type[User]] | None]


class ShapeStage(BaseModel):
    """Data defining a shape stage."""

    run_time: int
    users: int
    spawn_rate: float
    user_classes: list[Type[User]] = [ContileFirefoxUser, ContileNonFirefoxUser]


class ContileLoadTestShape(LoadTestShape):
    """A load test shape class for Contile (Duration: 10 minutes, Users: 200)."""

    RUN_TIME: int = 600  # 10 minutes (must not be set to less than 1 minute)
    USERS = 200

    stages: list[ShapeStage]

    def __init__(self):
        super(LoadTestShape, self).__init__()

        spawn_rate: float = round(self.USERS / 60, 2)
        self.stages = [
            # Stage 1: Spawn users in the first minute and dwell until the last minute
            ShapeStage(
                run_time=(self.RUN_TIME - 60),
                users=self.USERS,
                spawn_rate=spawn_rate,
            ),
            # Stage 2: Stop users in the last minute
            ShapeStage(run_time=self.RUN_TIME, users=0, spawn_rate=spawn_rate),
        ]

    def tick(self) -> TickTuple | None:
        """Override defining the desired distribution for Contile load testing.

        Returns:
            TickTuple: Distribution parameters
                user_count: Total user count
                spawn_rate: Number of users to start/stop per second when changing
                            number of users
                user_classes: None or a List of user classes to be spawned
            None: Instruction to stop the load test
        """
        for stage in self.stages:
            if self.get_run_time() < stage.run_time:
                return stage.users, stage.spawn_rate, stage.user_classes
        return None
