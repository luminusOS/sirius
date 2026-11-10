#!/usr/bin/env python3
"""Derives the RPM Version and Release from a release tag and checks them against Cargo.toml.

A final tag v1.2.3 gives Version 1.2.3. A release candidate v1.2.3-rc.1 (or
-rc1) gives Version 1.2.3~rc.1. RPM sorts a tilde before everything else, so the
candidate is older than the final release and DNF offers the final build as an
update to a candidate. This is the form the Luminus distribution contract uses.
"""
import pathlib
import re
import sys
import tomllib

TAG = re.compile(r"^v(?P<base>(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*))(?:-rc\.?(?P<rc>[1-9][0-9]*))?$")


def derive(tag, cargo_version):
    match = TAG.fullmatch(tag)
    if not match:
        raise ValueError(f"invalid release tag: {tag}")
    if match["base"] != cargo_version:
        raise ValueError(f"tag {tag} does not match workspace version {cargo_version}")
    version = f"{match['base']}~rc.{match['rc']}" if match["rc"] else match["base"]
    return version, "1"


def main(argv):
    if len(argv) != 3:
        print("usage: rpm_release.py TAG CARGO_TOML", file=sys.stderr)
        return 2
    cargo = tomllib.loads(pathlib.Path(argv[2]).read_text())["workspace"]["package"]["version"]
    try:
        version, release = derive(argv[1], cargo)
    except ValueError as error:
        print(error, file=sys.stderr)
        return 1
    print(f"version={version}")
    print(f"release={release}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
