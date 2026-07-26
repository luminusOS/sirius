#!/usr/bin/env python3
"""Generate data/images/timezone-map.svg from Natural Earth 110m land GeoJSON.

The projection matches the pixel math in crates/sirius-app/src/pages/timezone.rs
(Miller cylindrical, cropped to 81°N..59°S, no longitude offset), so pins and
the meridian band project onto the artwork exactly. Style mirrors the
gnome-initial-setup map: white land on GNOME blue (#62a0ea) ocean.

Usage: generate-timezone-map.py ne_110m_land.geojson > ../data/images/timezone-map.svg
Natural Earth data is public domain (https://www.naturalearthdata.com/).
"""

import json
import math
import sys

WIDTH = 1600
HEIGHT = 818  # 2x the gnome-initial-setup bg.png (800x409) for HiDPI
TOP_LATITUDE = 81.0
BOTTOM_LATITUDE = -59.0
MILLER_FULL_RANGE = 4.60682508676

OCEAN = "#62a0ea"
LAND = "#ffffff"


def miller(latitude: float) -> float:
    return 1.25 * math.log(math.tan(math.pi / 4 + 0.4 * math.radians(latitude)))


TOP_OFFSET = MILLER_FULL_RANGE * (TOP_LATITUDE / 180.0)
MAP_RANGE = abs(miller(BOTTOM_LATITUDE) - TOP_OFFSET)


def project(longitude: float, latitude: float) -> tuple[float, float]:
    x = (longitude + 180.0) / 360.0 * WIDTH
    y = abs(miller(latitude) - TOP_OFFSET) / MAP_RANGE * HEIGHT
    return x, y


def ring_path(ring: list) -> str:
    parts = []
    for longitude, latitude in ring:
        x, y = project(longitude, latitude)
        parts.append(f"{x:.1f},{y:.1f}")
    return "M" + "L".join(parts) + "Z"


def geometry_paths(geometry: dict) -> list[str]:
    polygons = (
        [geometry["coordinates"]]
        if geometry["type"] == "Polygon"
        else geometry["coordinates"]
    )
    paths = []
    for polygon in polygons:
        # Drop Antarctica and anything else fully below the map's crop.
        if all(lat < -58 for ring in polygon for _, lat in ring):
            continue
        paths.extend(ring_path(ring) for ring in polygon)
    return paths


def main() -> None:
    with open(sys.argv[1]) as source:
        geojson = json.load(source)

    paths = []
    for feature in geojson["features"]:
        paths.extend(geometry_paths(feature["geometry"]))

    print(
        f'<svg xmlns="http://www.w3.org/2000/svg" '
        f'width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">'
    )
    print(f'<rect width="{WIDTH}" height="{HEIGHT}" fill="{OCEAN}"/>')
    print(f'<path fill="{LAND}" d="{" ".join(paths)}"/>')
    print("</svg>")


if __name__ == "__main__":
    main()
