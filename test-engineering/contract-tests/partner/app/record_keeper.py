from collections import Counter
from multiprocessing.managers import SyncManager
from typing import List, Tuple

from fastapi import Request

from models import Header, QueryParameter, Record, RecordCount, Records


class RecordKeeper:
    """Responsible for Contile request history management"""

    def __init__(self, multi_process_manager: SyncManager) -> None:
        """Create an instance of RecordKeeper."""

        self._records: List[Record] = multi_process_manager.list()

    def add(self, request: Request) -> None:
        """Create record from Fast API Request and add record to the record keeper."""

        headers: Tuple[Header, ...] = tuple(
            Header(name=name, value=value) for name, value in request.headers.items()
        )

        query_parameters: Tuple[QueryParameter, ...] = tuple(
            QueryParameter(name=name, value=value)
            for name, value in request.query_params.multi_items()
        )

        record: Record = Record(
            method=request.method,
            headers=headers,
            path=request.url.path,
            query_parameters=query_parameters,
        )

        self._records.append(record)

    def clear(self) -> None:
        """Remove all records from the record keeper."""

        self._records[:] = []

    def get_all(self) -> Records:
        """Return all records in the record keeper with a counter."""

        records: List[RecordCount] = [
            RecordCount(count=count, record=record)
            for record, count in Counter(self._records).items()
        ]

        return Records(records=records)
