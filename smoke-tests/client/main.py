# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

from dataclasses import asdict, dataclass, field
from enum import Enum
from typing import Dict, Optional, Union

import requests
from flask import Request, Response, abort, jsonify

# See https://github.com/mozilla-services/contile/blob/main/src/web/dockerflow.rs
LOCATION_ENDPOINT: str = "__loc_test__"


@dataclass
class LocResponseData:
    """Location data returned by Contile."""

    country: str
    ip: str
    provider: str
    region: Optional[str]


# See https://github.com/mozilla-services/contile/blob/main/docs/API.md
class Environments(Enum):
    """Enum with accepted Contile environments."""

    DEV: str = "https://contile-dev.topsites.nonprod.cloudops.mozgcp.net/"
    STAGE: str = "https://contile-stage.topsites.nonprod.cloudops.mozgcp.net/"
    PROD: str = "https://contile.services.mozilla.com/"


@dataclass
class RequestData:
    """Data in the HTTP request to the HTTP Cloud Function."""

    environment: str
    expected_country: str
    expected_region: str


# Type alias for location data in errors
LocationData = Dict[str, Optional[str]]


@dataclass
class Error:
    """Information about an error that occured."""

    url: str
    message: str
    want: Union[LocationData, int]
    got: Union[LocationData, int]
    extra: Dict = field(default_factory=dict)


@dataclass
class ResponseData:
    """Data in the HTTP response."""

    error: Optional[Error] = None


def run_geo_smoke_test(request: Request):
    """Triggered by HTTP Cloud Function."""

    if request.method != "POST":
        return abort(Response("Only HTTP POST requests are allowed", status=405))

    try:
        request_data = RequestData(**request.get_json())
    except TypeError:
        return abort(Response("Invalid request data", status=400))

    try:
        env = Environments[request_data.environment]
    except KeyError:
        return abort(Response("Invalid environment parameter", status=400))

    location_url = f"{env.value}{LOCATION_ENDPOINT}"

    loc_response = requests.get(location_url)

    if loc_response.status_code != 200:
        error = Error(
            url=location_url,
            message="Unexpected status code",
            want=200,
            got=loc_response.status_code,
        )
        return jsonify(asdict(ResponseData(error=error)))

    loc_response_data = LocResponseData(**loc_response.json())

    want: LocationData = {
        "country": request_data.expected_country,
        "region": request_data.expected_region,
        "provider": "maxmind",
    }

    got: LocationData = {
        "country": loc_response_data.country,
        "region": loc_response_data.region,
        "provider": loc_response_data.provider,
    }

    if got != want:
        error = Error(
            url=location_url,
            message="Unexpected geolocation information",
            want=want,
            got=got,
            extra={"ip": loc_response_data.ip},
        )
        return jsonify(asdict(ResponseData(error=error)))

    return jsonify(asdict(ResponseData(error=None)))
