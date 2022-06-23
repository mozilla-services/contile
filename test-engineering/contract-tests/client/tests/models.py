# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

from typing import Any, List, Optional, Union

from pydantic import BaseModel, Extra


class Header(BaseModel):
    """Class that holds information about a HTTP header."""

    name: str
    value: str


class Request(BaseModel):
    """Class that holds information about a HTTP request to Contile."""

    method: str
    path: str
    headers: List[Header] = []


class Tile(BaseModel, extra=Extra.allow):
    """Class that holds information about a Tile returned by Contile."""

    id: int
    name: str
    click_url: str
    image_url: str
    image_size: Optional[int]
    impression_url: str
    url: str
    new_field: str


class Tiles(BaseModel):
    """Class that contains a list of Tiles returned by Contile."""

    tiles: List[Tile]


class Response(BaseModel):
    """Class that holds information about a HTTP response from Contile."""

    status_code: int
    content: Union[Tiles, Any]
    headers: List[Header] = []


class Step(BaseModel):
    """Class that holds information about a step in a test scenario."""

    request: Request
    response: Response


class Scenario(BaseModel):
    """Class that holds information about a specific test scenario."""

    name: str
    description: str
    steps: List[Step]
