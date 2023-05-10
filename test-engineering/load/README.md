# Contile Load (Locust) Tests

This directory contains the automated load test suite for the Mozilla Tile Service (MTS)
or Contile. The load test framework was originally developed in isolation using 
Molotov, see [contile-loadtests][1].

## Related Documentation

* [Contile Load Test History][13]
* [Contile Load Test Spreadsheet][11]

## Contributing

This project uses [Poetry][2] for dependency management. For environment setup it is 
recommended to use [pyenv][3] and [pyenv-virtualenv][4], as they work nicely with 
Poetry.

Project dependencies are listed in the `pyproject.toml` file.
To install the dependencies execute:
```shell
poetry install
```

Contributors to this project are expected to execute the following tools for import 
sorting, linting, style guide enforcement and static type checking.
Configurations are set in the `pyproject.toml` and `.flake8` files.

**[isort][5]**
 ```shell
poetry run isort common locustfiles
 ```

**[black][6]**
 ```shell
poetry run black common locustfiles
 ```

**[flake8][7]**
 ```shell
poetry run flake8 common locustfiles
 ```

**[mypy][8]**
```shell
poetry run mypy common locustfiles
```

## Opt-In Execution in Staging (and Production)

To automatically kick off load testing in staging along with your pull request commit, you have to include
a label in your git commit. This must be the merge commit on the `main` branch, since only the most recent commit is checked for the label. This label is in the form of: `[load test: (abort|warn)]`. Take careful note
of correct syntax and spacing within the label. There are two options for load tests, being `abort` and `warn`.

The `abort` label will prevent a `prod` deployment should the load test fail.
Ex. `feat: Add feature ABC [load test: abort]`.

The `warn` label will output a Slack warning should the load test fail, but still allow for `prod` deployment.
Ex. `feat: Add feature XYZ [load test: warn]`.

The commit tag signals load test instructions to Jenkins by modifying the Docker image tag. The Jenkins deployment workflow first deploys to `stage` and then runs load tests if requested. The Docker image tag passed to Jenkins appears as follows:
`^(?P<environment>stage|prod)(?:-(?P<task>\w+)-(?P<onfailure>warn|abort))?-(?P<commit>[a-z0-9]+)$`.

The `docker-image-publish-stage` and `docker-image-publish-prod` jobs in [.circleci/config.yml](/.circleci/config.yml) define this process so you can view the steps there for more clarity.

## Local Execution

Follow the steps bellow to execute the load tests locally:

### Setup Environment

#### 1. Configure Environment Variables

Environment variables, listed bellow or specified by [Locust][14], can be set in 
`test-engineering\load\docker-compose.yml`.

| Environment Variable                             | Node(s)         | Description                                                                                                              |
|--------------------------------------------------|-----------------|--------------------------------------------------------------------------------------------------------------------------|
| (*OPTIONAL*) CONTILE_LOCATION_TEST_HEADER        | master & worker | The HTTP header used to manually specify the location from which the request originated (defaults to `X-Test-Location`). |


#### 2. Host Locust via Docker

Execute the following from the `test-engineering\load` directory:
```shell
docker-compose -f docker-compose.yml -p contile-py-load-tests up --scale locust_worker=1
```

### Run Test Session

#### 1. Start Load Test

* In a browser navigate to `http://localhost:8089/`
* Set up the load test parameters:
  * Option 1: Select the `Default` load test shape with the following recommended settings:
    * Number of users: 200
    * Spawn rate: 3
    * Host: 'http://localhost:8000' 
    * Duration (Optional): 10m
  * Option 2: Select the `ContileLoadTestShape`
    * This option has pre-defined settings and will last 10 minutes
* Select "Start Swarming"

#### 2. Stop Load Test

Select the 'Stop' button in the top right hand corner of the Locust UI, after the 
desired test duration has elapsed. If the 'Run time' or 'Duration' is set in step 1, 
the load test will stop automatically.

#### 3. Analyse Results

* See [Distributed GCP Execution - Analyse Results](#3-analyse-results-1)
* Only client-side measures, provided by Locust, are available for local execution

### Clean-up Environment

#### 1. Remove Load Test Docker Containers

Execute the following from the `test-engineering\load` directory:
```shell
docker-compose -f docker-compose.yml -p contile-py-load-tests down
docker rmi locust
```

### Debugging

See [Locust - Running tests in a debugger][15]

## Distributed GCP Execution

Follow the steps bellow to execute the distributed load tests on GCP:

### Setup Environment

#### 1. Start a GCP Cloud Shell

The load tests can be executed from the [contextual-services-test-eng cloud shell][9].

#### 2. Configure the Bash Script

* The `setup_k8s.sh` file, located in the `test-engineering\load` directory, contains
shell commands to **create** a GKE cluster, **setup** an existing GKE cluster or
**delete** a GKE cluster
  * Execute the following from the `load` directory, to make the file executable:
    ```shell
    chmod +x setup_k8s.sh
    ```

#### 3. Create the GCP Cluster

* Execute the `setup_k8s.sh` file and select the **create** option, in order to 
  initiate the process of creating a cluster, setting up the env variables and 
  building the docker image
  ```shell
  ./setup_k8s.sh
  ```
* The cluster creation process will take some time. It is considered complete, once 
  an external IP is assigned to the `locust_master` node. Monitor the assignment via
  a watch loop:
  ```bash
  kubectl get svc locust-master --watch
  ```
* The number of workers is defaulted to 5, but can be modified with the 
  `kubectl scale` command. Example (10 workers):
  ```bash
  kubectl scale deployment/locust-worker --replicas=10
  ```

### Run Test Session

#### 1. Start Load Test

* In a browser navigate to `http://$EXTERNAL_IP:8089`
   
  This url can be generated via command
  ```bash
  EXTERNAL_IP=$(kubectl get svc locust-master -o jsonpath="{.status.loadBalancer.ingress[0].ip}")
  echo http://$EXTERNAL_IP:8089
  ```

* Select "Start Swarming"
   * The load test is set up using `ContileLoadTestShape`, which has pre-defined settings and will last 10 minutes

#### 2. Stop Load Test

Select the 'Stop' button in the top right hand corner of the Locust UI, after the 
desired test duration has elapsed. If the 'Run time' or 'Duration' is set in step 1, 
the load test will stop automatically.

#### 3. Analyse Results

**RPS**
* The request-per-second load target for Contile is `3000`
* Locust reports client-side RPS via the "contile_stats.csv" file and the UI 
  (under the "Statistics" tab or the "Charts" tab)
* [Grafana][10] reports the server-side RPS via the 
  "HTTP requests per second per country" chart

**HTTP Request Failures** 
* The number of responses with errors (5xx response codes) should be `0`
* Locust reports Failures via the "contile_failures.csv" file and the UI 
  (under the "Failures" tab or the "Charts" tab)
* [Grafana][10] reports Failures via the "HTTP Response codes" chart and the
  "HTTP 5xx error rate" chart

**Exceptions**
* The number of exceptions raised by the test framework should be `0`
* Locust reports Exceptions via the "contile_exceptions.csv" file and the UI 
  (under the "Exceptions" tab)

#### 4. Report Results

* Results should be recorded in the [Contile Load Test Spreadsheet][11]
* Optionally, the Locust reports can be saved and linked in the spreadsheet:
  * Download the results via command:
      ```bash
      kubectl cp <master-pod-name>:/home/locust/contile_stats.csv contile_stats.csv
      kubectl cp <master-pod-name>:/home/locust/contile_exceptions.csv contile_exceptions.csv
      kubectl cp <master-pod-name>:/home/locust/contile_failures.csv contile_failures.csv
      ```
    The `master-pod-name` can be found at the top of the pod list:
      ```bash 
      kubectl get pods -o wide
      ```
  * Upload the files to [gist][12] and record the links

### Clean-up Environment

#### 1. Delete the GCP Cluster

Execute the `setup_k8s.sh` file and select the **delete** option
```shell
./tests/load/setup_k8s.sh
```

[1]: https://github.com/mozilla-services/contile-loadtests
[2]: https://python-poetry.org/docs/#installation
[3]: https://github.com/pyenv/pyenv#installation
[4]: https://github.com/pyenv/pyenv-virtualenv#installation
[5]: https://pycqa.github.io/isort/
[6]: https://black.readthedocs.io/en/stable/
[7]: https://flake8.pycqa.org/en/latest/
[8]: https://mypy-lang.org/
[9]: https://console.cloud.google.com/home/dashboard?q=search&referrer=search&project=spheric-keel-331521&cloudshell=false
[10]: https://earthangel-b40313e5.influxcloud.net/d/oak1zw6Gz/contile-infrastructure?orgId=1&refresh=1m&var-environment=stage
[11]: https://docs.google.com/spreadsheets/d/1lGHu--eXEy6ShErmU1-SQ26yLY4xOjGubnRXt0DdGB0/
[12]: https://gist.github.com/new
[13]: https://docs.google.com/document/d/10Hx4cGvGBvq0z0uOK_CG3ZcyaQnT_EtgR6MYXmIvG6Q/
[14]: https://docs.locust.io/en/stable/configuration.html#environment-variables
[15]: https://docs.locust.io/en/stable/running-in-debugger.html