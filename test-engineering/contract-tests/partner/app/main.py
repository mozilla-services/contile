# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import json
import logging
import os
import pathlib
import sys
import time

import yaml
from fastapi import FastAPI, Query, Response, status
from fastapi.encoders import jsonable_encoder
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
    sub2: str = Query(..., example="placement1"),
    country_code: str = Query(..., alias="country-code", example="US"),
    region_code: str = Query(..., alias="region-code", example="NY"),
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
        time.sleep(delay)

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
