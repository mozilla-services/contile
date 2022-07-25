# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

from requests import Response as RequestsResponse


class PartnerError(Exception):
    """Error specific to partner service interactions."""


class PartnerRecordsNotClearedError(PartnerError):
    """Error clearing partner records."""

    def __init__(self, response: RequestsResponse):
        error_message: str = (
            f"The Partner records may not have cleared after the test execution.\n"
            f"Response details:\n"
            f"Status Code: {response.status_code}\n"
            f"Content: '{response.text}'"
        )
        super().__init__(error_message)
