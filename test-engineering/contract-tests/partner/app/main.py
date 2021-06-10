# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import json
import logging
import pathlib
import sys
from typing import List

import yaml
from fastapi import FastAPI, Query
from pydantic import BaseModel
from pydantic.json import pydantic_encoder

logger = logging.getLogger("partner")

version = f"{sys.version_info.major}.{sys.version_info.minor}"

with pathlib.Path("app/tiles.yml").open() as f:
    available_tiles = yaml.safe_load(f)

app = FastAPI()


class Tile(BaseModel):
    id: int
    name: str
    click_url: str
    image_url: str
    impression_url: str
    advertiser_url: str


class TilesResponse(BaseModel):
    tiles: List[Tile]


@app.get("/")
async def read_root():
    message = (
        f"Hello world! From FastAPI running on Uvicorn "
        f"with Gunicorn. Using Python {version}"
    )
    return {"message": message}


@app.get("/tilesp", response_model=TilesResponse)
async def read_tilesp(
    partner: str = Query(..., example="demofeed"),
    sub1: str = Query(..., example="123456789"),
    sub2: str = Query(..., example="placement1"),
    country_code: str = Query(..., alias="country-code", example="US"),
    region_code: str = Query(..., alias="region-code", example="NY"),
    form_factor: str = Query(..., alias="form-factor", example="desktop"),
    os_family: str = Query(..., alias="os-family", example="macOS"),
    v: str = Query(..., example="1.0"),
    out: str = Query("json", example="json"),
    results: int = Query(1, example=2),
):
    """Endpoint for requests from Contile."""
    tiles = TilesResponse(tiles=available_tiles[:results])
    logger.debug(json.dumps(tiles, default=pydantic_encoder))
    return tiles
