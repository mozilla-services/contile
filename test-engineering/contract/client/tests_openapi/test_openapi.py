import os

import schemathesis

openapi_file = os.environ["OPENAPI_FILE"]
contile_url = os.environ["CONTILE_URL"]
schema = schemathesis.from_path(openapi_file, base_url=contile_url)


def check_non_firefox_ua(response, case) -> None:
    if response.status_code != 403:
        raise AssertionError("Service did not return 403 for non-Firefox user-agent")


def check_firefox_ua(response, case) -> None:
    if response.status_code != 200:
        raise AssertionError("Service did not return 200 for Firefox user-agent")


@schema.parametrize()
def test_api_from_firefox(case):
    response = case.call_and_validate(
        headers={"User-Agent": "Mozilla/5.0 (Windows NT 10.0; rv:10.0) Firefox/91.0"}
    )
    case.validate_response(response, additional_checks=(check_firefox_ua,))


@schema.parametrize()
def test_api_not_from_firefox(case):
    response = case.call_and_validate()
    case.validate_response(response, additional_checks=(check_non_firefox_ua,))
