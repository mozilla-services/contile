from multiprocessing.managers import SyncManager
from typing import List

from fastapi import Request

from models import Header, Record, Records


class RecordKeeper:
    """Responsible for Contile request history management"""

    def __init__(self, multi_process_manager: SyncManager) -> None:
        """Create an instance of RecordKeeper."""

        self._records: List[Record] = multi_process_manager.list()

    def add(self, request: Request) -> None:
        """Create record from Fast API Request and add record to the record keeper."""

        headers: List[Header] = [
            Header(name=name, value=value) for name, value in request.headers.items()
        ]
        record: Record = Record(
            method=request.method,
            headers=headers,
            path=request.url.path,
            query=request.url.query,
        )
        self._records.append(record)

    def clear(self) -> None:
        """Remove all records from the record keeper."""

        self._records[:] = []

    def get_all(self) -> Records:
        """Return all records in the record keeper."""

        return Records(records=list(self._records))
