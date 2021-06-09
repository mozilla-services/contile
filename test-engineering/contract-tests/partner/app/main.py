# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import json
import logging
import sys
from typing import List

from fastapi import FastAPI, Query
from pydantic import BaseModel
from pydantic.json import pydantic_encoder

logger = logging.getLogger("partner")

version = f"{sys.version_info.major}.{sys.version_info.minor}"

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


TILES = [
    Tile(
        id=12345,
        name="Example",
        click_url="https://example.com/ctp?version=16.0.0&key=22.1&ci=6.2&ctag=1612376952400200000",
        image_url="https://example.com/image_url.jpg",
        impression_url="https://example.com/impression_url?id=0001",
        advertiser_url="https://www.example.com/advertiser_url",
    ),
    Tile(
        id=56789,
        name="Example",
        click_url="https://example.com/ctp?version=16.0.0&key=7.2&ci=8.9&ctag=E1DE38C8972D0281F5556659A",
        image_url="https://example.com/image_url.jpg",
        impression_url="https://example.com/impression_url?id=0002",
        advertiser_url="https://www.example.com/advertiser_url",
    ),
    Tile(
        id=11111,
        name="Example",
        click_url="https://example.com/ctp?version=16.0.0&key=3.3&ci=4.4&ctag=1612376952400200000",
        image_url="https://example.com/image_url.jpg",
        impression_url="https://example.com/impression_url?id=0003",
        advertiser_url="https://www.example.com/advertiser_url",
    ),
]


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
    tiles = TilesResponse(tiles=TILES[:results])
    logger.debug(json.dumps(tiles, default=pydantic_encoder))
    return tiles
