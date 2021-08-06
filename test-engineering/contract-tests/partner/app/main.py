# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import asyncio
import json
import logging
import os
import pathlib
import sys

import yaml
from fastapi import FastAPI, Query, Request, Response, status
from fastapi.encoders import jsonable_encoder
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from models import ResponseFromFile, Tiles

logger = logging.getLogger("partner")

version = f"{sys.version_info.major}.{sys.version_info.minor}"

RESPONSES_FILE = os.environ["RESPONSES_FILE"]

with pathlib.Path(RESPONSES_FILE).open() as f:
    responses_yml = yaml.safe_load(f)

responses_from_file = {
    form_factor: {
        os_family: ResponseFromFile(**response)
        for os_family, response in os_families.items()
    }
    for form_factor, os_families in responses_yml.items()
}

app = FastAPI()


@app.exception_handler(RequestValidationError)
async def validation_exception_handler(
    request: Request, exc: RequestValidationError
) -> Response:
    """Custom validation exception handler that returns a 400 Bad Request.

    This is required to match the partner API implementation.
    """
    body_from_API_spec = {
        "status": {"code": "103", "text": "Invalid input"},
        "count": "0",
        "response": "1",
    }

    # Include the example response body from the API spec in the response in
    # case contile is processing that information internally. Return the actual
    # validation error from FastAPI under the key "test".
    return JSONResponse(
        status_code=status.HTTP_400_BAD_REQUEST,
        content=jsonable_encoder(
            {"test": {"detail": exc.errors(), "body": exc.body}, **body_from_API_spec}
        ),
    )


@app.get("/")
async def read_root():
    message = (
        f"Hello world! From FastAPI running on Uvicorn "
        f"with Gunicorn. Using Python {version}"
    )
    return {"message": message}


@app.get("/tilesp", response_model=Tiles, status_code=200)
async def read_tilesp(
    response: Response,
    partner: str = Query(..., example="demofeed"),
    sub1: str = Query(..., example="123456789"),
    sub2: str = Query(
        ..., example="placement1", max_length=128, regex="^[a-zA-Z0-9]+$"
    ),
    country_code: str = Query(
        ..., alias="country-code", example="US", length=2, regex="^[A-Z]{2}$"
    ),
    region_code: str = Query(
        ..., alias="region-code", example="NY", regex="^[A-Z0-9]{1,3}$"
    ),
    form_factor: str = Query(..., alias="form-factor", example="desktop"),
    os_family: str = Query(..., alias="os-family", example="macos"),
    v: str = Query(..., example="1.0"),
    out: str = Query("json", example="json"),
    results: int = Query(1, example=2),
):
    """Endpoint for requests from Contile."""
    # Read response information from the response.yml file
    response_from_file = responses_from_file[form_factor][os_family]

    # Read response information from the file
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
