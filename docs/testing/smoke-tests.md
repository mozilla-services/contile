# Smoke Tests

This documentation describes the smoke test suite for Contile deployments.

## Setup

Set runtime environment variables for the `runner` function that contain trigger
URLs for the clients using the following naming scheme:

`CLIENT_URL_<COUNTRY_CODE>`

Examples:
```
CLIENT_URL_US
CLIENT_URL_GB
CLIENT_URL_CH
```

## Example runner invocation

```bash
curl -m 70 -X POST <RUNNER_TRIGGER_URL> \
-H "Authorization:bearer $(gcloud auth print-identity-token)" \
-H "Content-Type:application/json" \
-d '{"environments": ["STAGE", "PROD"]}'
```

## Deployment

Smoke tests are executed in the CD pipeline and deployed manually by SRE: 
[Terraform Configuration][terraform].

[terraform]: https://github.com/mozilla-services/cloudops-infra/tree/master/projects/topsites/tf/modules/geolocation-smoke-tests