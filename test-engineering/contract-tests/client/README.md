# client

This directory contains a Python-based test framework for the contract tests.
The HTTP client used in the framework supports:

* Requests for tiles from the MTS, with response checks. 
* Requests for the history of requests from the MTS to the partner API with response 
checks.

The framework implements response models for the MTS and partner APIs.

For more details on contract test design, refer to the contile-contract-tests
[README][contract_tests_readme].

## Scenarios

The client is instructed on request and response check actions via steps recorded in a 
scenario file. A scenario is defined by a name, description and steps.

### Steps

#### Contile Service

* To direct requests to the MTS service, set the `service` value of `request` to 
`contile`
* The expected content for a `200 OK` response is a collection of tiles.

Example:
```yaml
- request:
    service: contile
    method: GET
    path: '/v1/tiles'
    headers:
      - name: User-Agent
        value: 'Mozilla/5.0 (Windows NT 10.0; rv:10.0) Gecko/20100101 Firefox/91.0'
  response:
    status_code: 200
    content:
      tiles:
        - id: 12345
          name: 'Example COM'
          click_url: 'https://example.com/desktop_windows?version=16.0.0&key=22.1&ci=6.2&ctag=1612376952400200000'
          image_url: 'https://example.com/desktop_windows01.jpg'
          image_size: null
          impression_url: 'https://example.com/desktop_windows?id=0001'
          url: 'https://www.example.com/desktop_windows'
        - id: 56789
          name: 'Example ORG'
          click_url: 'https://example.org/desktop_windows?version=16.0.0&key=7.2&ci=8.9&ctag=E1DE38C8972D0281F5556659A'
          image_url: 'https://example.org/desktop_windows02.jpg'
          image_size: null
          impression_url: 'https://example.org/desktop_windows?id=0002'
          url: 'https://www.example.org/desktop_windows'
```

#### Partner Service

* To direct requests to the partner service, set the `service` value of `request` to 
`partner`
* The expected content for a `200 OK` response is a collection of records.
    * Each `record` represents a distinct request made by the MTS to the partner.
    * The frequency of a request is denoted by the `count`.
* Request history is cleared between scenarios

Example:
```yaml
- request:
    service: partner
    method: GET
    path: '/records/'
    headers:
      - name: 'accept'
        value: '*/*'
  response:
    status_code: 200
    content:
      records:
        - count: 1
          record:
            method: GET
            headers:
              - name: accept
                value: '*/*'
              - name: user-agent
                value: 'contile/1.8.0'
              - name: host
                value: 'partner:5000'
            path: '/tilesp/desktop'
            query_parameters:
              - name: partner
                value: 'partner_id_test'
              - name: sub1
                value: 'sub1_test'
              - name: sub2
                value: 'newtab'
              - name: country-code
                value: 'US'
              - name: region-code
                value: ''
              - name: dma-code
                value: ''
              - name: form-factor
                value: 'desktop'
              - name: os-family
                value: 'windows'
              - name: v
                value: '1.0'
              - name: out
                value: 'json'
              - name: results
                value: '5'
```

[contract_tests_readme]: ../README.md
