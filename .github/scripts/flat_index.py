"""Write a flat package index linking every wheel attached to this fork's releases.

usage: flat_index.py OWNER/REPO OUTPUT_DIR

Each link carries the wheel's SHA-256, computed here from the file GitHub serves, so an installer
reading the index refuses any wheel whose bytes changed after the release.
"""

import hashlib
import html
import json
import os
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

RELEASE_TAG = re.compile(r"v(\d+\.\d+\.\d+)-oxidian\.(\d+)")


def get(url: str, accept: str) -> urllib.request.addinfourl:
    request = urllib.request.Request(url, headers={"Accept": accept})
    token = os.environ.get("GH_TOKEN")
    if token and url.startswith("https://api.github.com/"):
        request.add_header("Authorization", f"Bearer {token}")
    return urllib.request.urlopen(request, timeout=120)


def releases(repo: str) -> list[dict]:
    found: list[dict] = []
    page = 1
    while True:
        url = f"https://api.github.com/repos/{repo}/releases?per_page=100&page={page}"
        with get(url, "application/vnd.github+json") as response:
            batch = json.load(response)
        if not batch:
            return found
        found.extend(batch)
        page += 1


def sha256_of(url: str) -> str:
    digest = hashlib.sha256()
    with get(url, "application/octet-stream") as response:
        while chunk := response.read(1 << 20):
            digest.update(chunk)
    return digest.hexdigest()


def main(repo: str, output: Path) -> None:
    links: list[tuple[str, str]] = []
    for release in releases(repo):
        tag = RELEASE_TAG.fullmatch(release["tag_name"])
        if release["draft"] or tag is None:
            continue
        prefix = f"pycrdt-{tag[1]}+oxidian.{tag[2]}-"
        for asset in release["assets"]:
            name = asset["name"]
            if not name.endswith(".whl"):
                continue
            if not name.startswith(prefix):
                raise SystemExit(f"{release['tag_name']}: wheel {name} does not start {prefix}")
            url = asset["browser_download_url"]
            if urllib.parse.unquote(url.rsplit("/", 1)[-1]) != name:
                raise SystemExit(f"{name} is served as {url}")
            sha256 = sha256_of(url)
            published = asset.get("digest")
            if published and published != f"sha256:{sha256}":
                raise SystemExit(f"{name}: GitHub reports {published}, download hashes {sha256}")
            links.append((name, f"{url}#sha256={sha256}"))
    if not links:
        raise SystemExit("no wheels found")
    links.sort()
    body = "\n".join(
        f'    <a href="{html.escape(href)}">{html.escape(name)}</a><br>' for name, href in links
    )
    output.mkdir(parents=True, exist_ok=True)
    (output / "index.html").write_text(
        "<!DOCTYPE html>\n"
        "<html>\n"
        "  <head>\n"
        '    <meta charset="utf-8">\n'
        f"    <title>{html.escape(repo)} wheels</title>\n"
        "  </head>\n"
        "  <body>\n"
        f"{body}\n"
        "  </body>\n"
        "</html>\n"
    )
    for name, href in links:
        print(name, href)


if __name__ == "__main__":
    main(sys.argv[1], Path(sys.argv[2]))
