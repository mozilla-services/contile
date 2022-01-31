# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import asyncio
import enum
import json
import logging
import os
import pathlib
import sys
from typing import Dict, List

from fastapi import FastAPI, Query, Request, Response, status
from fastapi.encoders import jsonable_encoder
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from models import Tiles
from responses import LoaderConfig, load_responses

logger = logging.getLogger("partner")

version = f"{sys.version_info.major}.{sys.version_info.minor}"

RESPONSES_DIR = pathlib.Path(os.environ["RESPONSES_DIR"])

scenarios_files = [p for p in RESPONSES_DIR.glob("**/*.yml")]

if not scenarios_files:
    raise RuntimeError(
        f"RESPONSES_DIR '{RESPONSES_DIR}' does not contain any YML files"
    )

LOADER_CONFIG = LoaderConfig(RESPONSES_DIR)

app = FastAPI()

# This is only included for client errors such as invalid query parameter values
# or unknown query parameters.
BODY_FROM_API_SPEC = {
    "status": {"code": "103", "text": "Invalid input"},
    "count": "0",
    "response": "1",
}


@app.exception_handler(RequestValidationError)
async def validation_exception_handler(
    request: Request, exc: RequestValidationError
) -> Response:
    """Custom validation exception handler that returns a 400 Bad Request.

    This is required to match the partner API implementation.
    """

    # Include the example response body from the API spec in the response in
    # case contile is processing that information internally. Return the actual
    # validation error from FastAPI under the key "test".
    return JSONResponse(
        status_code=status.HTTP_400_BAD_REQUEST,
        content=jsonable_encoder(
            {"test": {"detail": exc.errors(), "body": exc.body}, **BODY_FROM_API_SPEC}
        ),
    )


@app.get("/")
async def read_root():
    message = (
        f"Hello world! From FastAPI running on Uvicorn "
        f"with Gunicorn. Using Python {version}"
    )
    return {"message": message}


# Make sure to update this when query parameters for `read_tilesp` change
ACCEPTED_QUERY_PARAMS = [
    "partner",
    "sub1",
    "sub2",
    "country-code",
    "region-code",
    "dma-code",
    "form-factor",
    "os-family",
    "v",
    "out",
    "results",
]


class Endpoint(str, enum.Enum):
    """Path parameters with pre-defined values for the supported endpoints."""

    mobile: str = "mobile"
    desktop: str = "desktop"


# Map from supported API endpoint path to accepted form-factor query parameter
# values. Example environment variables: 'phone,tablet' or 'desktop'.
FORM_FACTORS: Dict[Endpoint, List[str]] = {
    Endpoint.mobile: [
        form_factor.strip().lower()
        for form_factor in os.environ["ACCEPTED_MOBILE_FORM_FACTORS"].split(",")
    ],
    Endpoint.desktop: [
        form_factor.strip().lower()
        for form_factor in os.environ["ACCEPTED_DESKTOP_FORM_FACTORS"].split(",")
    ],
}


@app.get("/tilesp/{endpoint}", response_model=Tiles, status_code=200)
async def read_tilesp(
    request: Request,
    response: Response,
    endpoint: Endpoint,
    partner: str = Query(..., example="demofeed"),
    sub1: str = Query(..., example="123456789"),
    sub2: str = Query(
        ..., example="placement1", max_length=128, regex="^[a-zA-Z0-9]+$"
    ),
    # country_code parameter follows ISO-3166 alpha-2 standard and validations
    # (https://en.wikipedia.org/wiki/ISO_3166-1_alpha-2)
    country_code: str = Query(
        ..., alias="country-code", example="US", length=2, regex="^[A-Z]{2}$"
    ),
    # region_code parameter follows ISO-3166-2 standard and validations
    # https://en.wikipedia.org/wiki/ISO_3166-2
    region_code: str = Query(
        ..., alias="region-code", example="NY", regex="^([A-Z0-9]{1,3})?$"
    ),
    # dma_code parameter represents a Designated Marketing Area code in the US.
    dma_code: str = Query(..., alias="dma-code", example="532", regex="^([0-9]+)?$"),
    form_factor: str = Query(..., alias="form-factor", example="desktop"),
    os_family: str = Query(..., alias="os-family", example="macos"),
    v: str = Query(..., example="1.0"),
    out: str = Query("json", example="json"),
    results: int = Query(1, example=2),
):
    """Endpoint for requests from Contile."""

    unknown_query_params: List[str] = [
        param for param in request.query_params if param not in ACCEPTED_QUERY_PARAMS
    ]

    if unknown_query_params:
        logger.error(
            "received unexpected query parameters from Contile: %s",
            unknown_query_params,
        )

        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content=jsonable_encoder(
                {
                    "test": {"unexpected query parameter": unknown_query_params},
                    **BODY_FROM_API_SPEC,
                }
            ),
        )

    if form_factor not in FORM_FACTORS[endpoint]:
        logger.error("received form-factor '%s' on %s API", form_factor, endpoint.name)

        return JSONResponse(
            status_code=status.HTTP_400_BAD_REQUEST,
            content=jsonable_encoder(
                {
                    "test": {
                        f"invalid form-factor for {endpoint.name} API": form_factor
                    },
                    **BODY_FROM_API_SPEC,
                }
            ),
        )

    # Load responses from the responses.yml file for the given country_code and
    # region_code. If that fails and the fallback behavior fails as well, this
    # will raise an Exception resulting in a 500 Internal Server Error
    responses_from_file = load_responses(
        config=LOADER_CONFIG, country_code=country_code, region_code=region_code
    )

    # Read the response for the given form_factor and os_family
    response_from_file = responses_from_file[form_factor][os_family]

    status_code = response_from_file.status_code
    content = response_from_file.content
    delay = response_from_file.delay
    headers = {header.name: header.value for header in response_from_file.headers}

    if delay:
        # Add an artificual delay to the handler
        logger.debug("response is delayed by %s seconds", delay)
        await asyncio.sleep(delay)

    logger.debug("response status_code: %s", status_code)
    logger.debug("response headers %s", json.dumps(headers))
    logger.debug(
        "response content: %s",
        json.dumps(content, default=jsonable_encoder),
    )

    if status_code == status.HTTP_500_INTERNAL_SERVER_ERROR:
        raise RuntimeError("Something went wrong")

    # Use this to trigger BadAdmResponse errors in Contile
    if not isinstance(content, Tiles):
        return JSONResponse(content=content, headers=headers)

    response.headers.update(headers)
    response.status_code = status_code
    return content
