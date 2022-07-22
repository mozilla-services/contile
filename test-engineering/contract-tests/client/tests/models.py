# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

from enum import Enum
from typing import Any, List, Optional, Union

from pydantic import BaseModel, Extra
from requests import Response as RequestsResponse


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
    headers: List[Header] = []


class QueryParameter(BaseModel):
    """Model that represents a HTTP query parameter."""

    name: str
    value: str


class Record(BaseModel):
    """Model that represents a request sent by Contile."""

    method: str
    headers: List[Header]
    path: str
    query_parameters: List[QueryParameter]


class RecordCount(BaseModel):
    """Model that represents the number of times a request is sent by Contile."""

    count: int
    record: Record


class Records(BaseModel):
    """Model for a list of requests sent by Contile and their send count."""

    records: List[RecordCount]


class Tile(BaseModel, extra=Extra.allow):
    """Class that holds information about a Tile returned by Contile."""

    id: int
    name: str
    click_url: str
    image_url: str
    image_size: Optional[int]
    impression_url: str
    url: str


class Tiles(BaseModel):
    """Class that contains a list of Tiles returned by Contile."""

    tiles: List[Tile]


class Response(BaseModel):
    """Class that holds information about a HTTP response from Contile."""

    status_code: int
    content: Union[Records, Tiles, Any]
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


class PartnerError(Exception):
    """Error specific to partner service interactions."""


class PartnerRecordsNotClearedError(PartnerError):
    """Error clearing partner records."""

    def __init__(self, response: RequestsResponse):
        error_message: str = (
            f"The Partner records may not have cleared after the test execution.\n"
            f"Response details:\n"
            f"Status Code: {response.status_code}\n"
            f"Content: '{response.text}'"
        )
        super().__init__(error_message)
