# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

from typing import Any, Dict, List, Tuple, Union

from pydantic import BaseModel


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

    tiles: List[Tile]


class Header(BaseModel):
    """Model that represents a HTTP header."""

    class Config:
        frozen = True

    name: str
    value: str


class ResponseFromFile(BaseModel):
    """Model that represents a Response as defined in responses.yml."""

    status_code: int
    headers: List[Header]
    content: Union[Tiles, Any]
    delay: float = 0.0


class QueryParameter(BaseModel):
    """Model that represents a HTTP query parameter."""

    class Config:
        frozen = True

    name: str
    value: str


class Record(BaseModel):
    """Model that represents a request sent by Contile."""

    class Config:
        frozen = True

    method: str
    headers: Tuple[Header, ...]
    path: str
    query_parameters: Tuple[QueryParameter, ...]


class RecordCount(BaseModel):
    """Model that represents the number of times a request is sent by Contile."""

    record: Record
    count: int


class Records(BaseModel):
    """Model for a list of requests sent by Contile and their send count."""

    records: List[RecordCount]
