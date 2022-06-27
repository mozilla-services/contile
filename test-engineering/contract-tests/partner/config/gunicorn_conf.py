# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

import os
import pathlib

import yaml

port = os.getenv("PORT", "8000")
root = os.getenv("ROOT", "")

accesslog = "-"
errorlog = "-"
workers = 4

with pathlib.Path(root + "config/logging.yml").open() as f:
    logconfig_dict = yaml.safe_load(f)
