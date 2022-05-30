# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import os
from dataclasses import asdict, dataclass, field
from enum import Enum
from typing import Any, Dict, List

import google.auth.transport.requests
import google.oauth2.id_token
import requests
from flask import Request, Response, abort, jsonify


@dataclass
class Client:
    """Geo information about a client."""

    country: str
    region: str
    gcp_region: str


# TODO: Update the following enum based on the regions where you deploy clients
# See https://cloud.google.com/functions/docs/locations
class Clients(Enum):
    """Enum with clients deployed as Cloud Functions."""

    US: Client = Client(country="US", region="OR", gcp_region="us-west1")
    GB: Client = Client(country="GB", region="ENG", gcp_region="europe-west2")
    CH: Client = Client(country="CH", region="ZH", gcp_region="europe-west6")


class Environments(Enum):
    """Enum with accepted Contile environments."""

    DEV: str = "DEV"
    STAGE: str = "STAGE"
    PROD: str = "PROD"


@dataclass
class ClientResponse:
    """Information about a response from a client function."""

    status_code: int
    content: Any


@dataclass
class RequestData:
    """Data in the HTTP request to the HTTP Cloud Function."""

    environments: List[str] = field(default_factory=list)


@dataclass
class ResponseData:
    """Data in the HTTP response."""

    results: Dict = field(default_factory=dict)


def get_id_token(audience):
    """Fetch an oauth2 ID token for triggering other functions."""

    auth_req = google.auth.transport.requests.Request()
    id_token = google.oauth2.id_token.fetch_id_token(auth_req, audience)

    return id_token


def run_geo_smoke_tests(request: Request):
    """Triggered by HTTP Cloud Function."""

    if request.method != "POST":
        return abort(Response("Only HTTP POST requests are allowed", status=405))

    try:
        request_data = RequestData(**request.get_json())
    except TypeError:
        return abort(Response("Invalid request data", status=400))

    try:
        environments: List[Environments] = [
            Environments[env_name] for env_name in request_data.environments
        ]
    except KeyError:
        return abort(Response("Invalid environment parameter", status=400))

    if not environments:
        return abort(Response("Require list of environments", status=400))

    response_data = ResponseData()

    for env in environments:
        response_data.results[env.name] = {}
        for client in Clients:
            url = os.environ[f"CLIENT_URL_{client.name}"]
            id_token = get_id_token(url)
            response = requests.post(
                url,
                json={
                    "environment": env.value,
                    "expected_country": client.value.country,
                    "expected_region": client.value.region,
                },
                headers={
                    "Authorization": f"Bearer {id_token}",
                    "Accept": "application/json",
                },
            )
            response_data.results[env.name][client.name] = ClientResponse(
                status_code=response.status_code, content=response.json()
            )

    return jsonify(asdict(response_data))
