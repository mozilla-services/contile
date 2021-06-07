# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import sys
from typing import List

from fastapi import FastAPI, Query
from pydantic import BaseModel

version = f"{sys.version_info.major}.{sys.version_info.minor}"

app = FastAPI()


class Tile(BaseModel):
    id: int
    name: str
    click_url: str
    image_url: str
    impression_url: str
    advertiser_url: str


TILES = [
    Tile(
        id=12345,
        name="tile 12345",
        click_url="example click_url",
        image_url="example image_url",
        impression_url="example impression_url",
        advertiser_url="example advertiser_url",
    ),
    Tile(
        id=56789,
        name="tile 56789",
        click_url="example click_url",
        image_url="example image_url",
        impression_url="example impression_url",
        advertiser_url="example advertiser_url",
    ),
    Tile(
        id=11111,
        name="tile 11111",
        click_url="example click_url",
        image_url="example image_url",
        impression_url="example impression_url",
        advertiser_url="example advertiser_url",
    ),
]


@app.get("/")
async def read_root():
    message = (
        f"Hello world! From FastAPI running on Uvicorn "
        f"with Gunicorn. Using Python {version}"
    )
    return {"message": message}


@app.get("/tilesp", response_model=List[Tile])
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
    return TILES[:results]
