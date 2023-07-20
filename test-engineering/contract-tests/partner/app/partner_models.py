# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

from typing import Any

from pydantic import BaseModel, ConfigDict


class Tile(BaseModel):
    """Model for a tile returned to Contile."""

    id: int
    name: str
    click_url: str
    image_url: str
    impression_url: str
    advertiser_url: str


class Tiles(BaseModel):
    """Model for a list of tiles returned to Contile."""

    tiles: list[Tile]


class Header(BaseModel):
    """Model that represents a HTTP header."""

    model_config = ConfigDict(frozen=True)

    name: str
    value: str


class ResponseFromFile(BaseModel):
    """Model that represents a Response as defined in responses.yml."""

    status_code: int
    headers: list[Header]
    content: Tiles | Any
    delay: float = 0.0


class QueryParameter(BaseModel):
    """Model that represents a HTTP query parameter."""

    model_config = ConfigDict(frozen=True)

    name: str
    value: str


class Record(BaseModel):
    """Model that represents a request sent by Contile."""

    model_config = ConfigDict(frozen=True)

    method: str
    headers: tuple[Header, ...]
    path: str
    query_parameters: tuple[QueryParameter, ...]


class RecordCount(BaseModel):
    """Model that represents the number of times a request is sent by Contile."""

    count: int
    record: Record


class Records(BaseModel):
    """Model for a list of requests sent by Contile and their send count."""

    records: list[RecordCount]
