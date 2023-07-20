# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

FROM python:3.11-slim

LABEL maintainer="Content Discovery Services (DISCO) Team <disco-team@mozilla.com>"

# Add gcc since there are no wheels for some packages for arm64/aarch64
RUN apt-get update && apt-get install -y git && \
  if [ -n "$(arch | grep 'arm64\|aarch64')" ]; then \
    apt install -y --no-install-recommends gcc python3-dev; \
  fi

ENV LANG=C.UTF-8
ENV PYTHONUNBUFFERED=1

ENV PYTHON_VENV=/venv
RUN python -m venv ${PYTHON_VENV}
ENV PATH="${PYTHON_VENV}/bin:${PATH}"

RUN python -m pip install --upgrade pip

# Setup poetry and install requirements
ENV POETRY_VIRTUALENVS_CREATE=false \
    POETRY_VERSION=1.5.1
RUN python -m pip install --no-cache-dir --quiet poetry
COPY pyproject.toml poetry.lock ./
RUN poetry install --without dev --no-interaction --no-ansi

RUN useradd --create-home locust
WORKDIR /home/locust

COPY ./data ./data
COPY ./common ./common
COPY ./locustfiles ./locustfiles

# Expose ports for the web UI and the locust master
EXPOSE 8089 5557

USER locust
ENTRYPOINT ["locust", "-f", "locustfiles/locustfile.py,locustfiles/load.py"]
