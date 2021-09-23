# Contile API Specification

Firefox Instant Find (Contile) is a service to deliver sponsored tiles to customers on the New Browser Tab page. It's a simple service with one endpoint. It uses the User Agent's IP address (exposed via the `X-Forwarded-For` header) and some platform information from the `User-Agent` header.

Servers

|environment | server |
|--|--|
|dev| https://contile-dev.topsites.nonprod.cloudops.mozgcp.net/ |
|stage| https://contile-stage.topsites.nonprod.cloudops.mozgcp.net/ |
|prod| https://contile.services.mozilla.com/ |

## Calls

### Fetch Tiles

Return candidate tiles. The following user information is captured and [shared](https://github.com/mozilla-services/contile/blob/main/src/adm/tiles.rs) with our advertising partners:

|item | description | derived from | normalization method |
|--|--|--|--|
|country_code |Unicode Common Locale Data Repository (CLDR) code for the country (e.g. "US", "CA", "GB", etc.) | IP address | resolved from IP address lookup database
region_code | CLDR for the major region, if available. (e.g. "VA", "BC", "KW", etc.) | IP address | resolved from IP address lookup database
dma | US based Designated Market Area (DMA) code for areas above 15K population | IP address | a configuration specified list of DMAs to ignore. If a DMA matches that list, it's not reported.
form_factor | Device form factor | UserAgent string | rough form factor of the user's device normalized to "Desktop", "Phone", "Tablet" (Currently, only iPad), and "Other"
os_family | User Operating System | UserAgent string | rough operating system name normalized to "Windows", "MacOS", "Linux", "IOs", "Android", "ChromeOS", "BlackBerry", "Other"

#### Call

```http
GET /v1/tiles
```

#### Parameters

None

#### Response

Returns a JSON structure containing the tile information. For example:

```json
{"tiles":[
    {
        "id":74301,
        "name":"Amazon",
        "url":"https://...",
        "click_url":"https://...",
        "image_url":"https://...",
        "image_size":200,
        "impression_url":"...",
        "position":1
    },
    {
        "id":74161,
        "name":"eBay",
        "url":"https://...",
        "click_url":"https://...",
        "image_url":"https://...",
        "image_size":200,
        "impression_url":"https://...",
        "position":2
    }
]}
```

### Docker Flow endpoints

As with most Mozilla Services, Contile supports Dockerflow endpoints

```http
GET /___heartbeat__
```

return if the service is operational

```http
GET /__lbheartbeat__
```

return if the service is available for load balancers

```http
GET /__version__
```

get installed version information

In addition, the following extended Dockerflow endpoints are enabled:

```http
GET /__error__
```

Force an error, used to test Sentry reporting. This has an optional parameter of `with_location=true` which will include detected IP location information in the Sentry error message.
