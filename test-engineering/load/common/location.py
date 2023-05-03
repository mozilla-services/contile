# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/.

"""Location client data module."""

from xml.etree import ElementTree as ET
from xml.etree.ElementTree import Element, ElementTree


def parse_subdivision_codes_file(cldr_subdivision_file_path: str) -> list[str]:
    """Get location CLDR subdivision codes.

    Args:
        cldr_subdivision_file_path: File path to the XML file of unicode CLDR
                                    subdivision codes
    Returns:
        list[str]: list of location data in the form of "Code, Subdivision"
    Raises:
        OSError: If the XML file of unicode CLDR subdivision codes can't be opened
        ParseError: If parsing of the XML file of unicode CLDR subdivision codes fails
    """
    tree: ElementTree = ET.parse(cldr_subdivision_file_path)
    root: Element = tree.getroot()
    locations: list[str] = []
    for subgroup in root.iter("subgroup"):
        code: str = subgroup.attrib["type"]
        subdivisions: list[str] = subgroup.attrib["contains"].split(" ")
        for subdivision in subdivisions:
            locations.append(f"{code}, {subdivision.upper()}")
    return locations
