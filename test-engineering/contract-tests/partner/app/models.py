# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

from typing import Any, List, Union

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

    name: str
    value: str


class ResponseFromFile(BaseModel):
    """Model that represents a Response as defined in responses.yml."""

    status_code: int
    headers: List[Header]
    content: Union[Tiles, Any]
    delay: float = 0.0
