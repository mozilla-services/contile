# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

FROM python:3.8-slim

LABEL maintainer "Raphael Pierzina <raphael@hackebrot.de>"

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONPATH=/app \
    PORT=8000

# see https://github.com/mozilla-services/Dockerflow
# add a non-privileged user for running the application
RUN groupadd --gid 10001 app && \
    useradd -g app --uid 10001 --shell /usr/sbin/nologin --create-home --home-dir /app app

WORKDIR /app

COPY requirements.txt requirements.txt

RUN python -m venv venv
RUN venv/bin/python -m pip install -U pip
RUN venv/bin/python -m pip install -r requirements.txt

COPY config/ config/

COPY entrypoint.sh entrypoint.sh
RUN chmod +x entrypoint.sh

COPY app/ /app/

RUN chown app:app /app
USER app

EXPOSE ${PORT}
ENTRYPOINT [ "./entrypoint.sh" ]
CMD ["gunicorn","-c", "config/gunicorn_conf.py", "-k", "uvicorn.workers.UvicornWorker", "main:app"]
