FROM python:3.11-slim

LABEL maintainer "Raphael Pierzina <raphael@hackebrot.de>"

ENV PYTHON_VENV=/venv
RUN python -m venv ${PYTHON_VENV}
ENV PATH="${PYTHON_VENV}/bin:${PATH}"

RUN python -m pip install --upgrade pip

# Setup poetry and extract requirements
RUN python -m pip install --no-cache-dir --quiet poetry
WORKDIR /tmp
COPY ./pyproject.toml ./poetry.lock /tmp/
RUN poetry export --no-interaction --output requirements.txt --without-hashes

WORKDIR /
RUN python -m pip install -r /tmp/requirements.txt

COPY ./client usr/src/client/
WORKDIR /usr/src/client

ENTRYPOINT [ "pytest" ]
