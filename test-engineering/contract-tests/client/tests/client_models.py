# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

from enum import Enum
from typing import Any

from pydantic import BaseModel, ConfigDict


class Service(Enum):
    """Enum with contract-test service options."""

    CONTILE: str = "contile"
    PARTNER: str = "partner"


class Header(BaseModel):
    """Class that holds information about a HTTP header."""

    name: str
    value: str


class Request(BaseModel):
    """Class that holds information about a HTTP request to Contile."""

    service: Service
    method: str
    path: str
    headers: list[Header] = []


class QueryParameter(BaseModel):
    """Model that represents a HTTP query parameter."""

    name: str
    value: str


class Record(BaseModel):
    """Model that represents a request sent by Contile."""

    method: str
    headers: list[Header]
    path: str
    query_parameters: list[QueryParameter]


class RecordCount(BaseModel):
    """Model that represents the number of times a request is sent by Contile."""

    count: int
    record: Record


class Records(BaseModel):
    """Model for a list of requests sent by Contile and their send count."""

    records: list[RecordCount]


class Tile(BaseModel):
    """Class that holds information about a Tile returned by Contile."""

    model_config = ConfigDict(extra="allow")

    id: int
    name: str
    click_url: str
    image_url: str
    image_size: int | None = None
    impression_url: str
    url: str


class TilesResponse(BaseModel):
    """Class that contains a list of Tiles and SOV string returned by Contile."""

    tiles: list[Tile]
    sov: str | None = None


class Response(BaseModel):
    """Class that holds information about a HTTP response from Contile."""

    status_code: int
    content: Records | TilesResponse | Any
    headers: list[Header] = []


class Step(BaseModel):
    """Class that holds information about a step in a test scenario."""

    request: Request
    response: Response


class Scenario(BaseModel):
    """Class that holds information about a specific test scenario."""

    name: str
    description: str
    steps: list[Step]
