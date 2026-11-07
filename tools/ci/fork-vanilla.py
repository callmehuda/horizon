#!/usr/bin/env python3
"""The pinned Minecraft's files, downloaded from Mojang's public endpoints for this fork's CI.

Upstream's exporters job runs on a runner that holds the maintainer's own Minecraft; a fork has
no such runner, so this script fetches the same pinned files from the endpoints every launcher
reads (piston-meta.mojang.com, piston-data.mojang.com, resources.download.minecraft.net):

  * the version manifest and the pinned version's JSON, checked against research-lock.json
    (manifest_sha1 and its accepted list, the client and server jar sha1s, the asset index);
  * the client jar and the server jar, each checked against its pinned sha1;
  * every library artifact of the version JSON, in the Maven layout, each checked against its
    sha1 (the importer picks the ones this system's rules allow; the rest are harmless);
  * the asset index and every object of it, in the official launcher's layout (the asset copy
    step of `horizon-prepare --prepare` reads them), each checked against its size.

Nothing here runs anything: files are downloaded and verified only, exactly as launcher/src/
mojang.rs does it for a player. Files that already exist with the right checksum are kept, so a
retry costs only what failed.

    python3 tools/ci/fork-vanilla.py --out /tmp/minecraft
"""

import argparse
import concurrent.futures
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
OBJECTS = "https://resources.download.minecraft.net"
JOBS = 16


def sha1(path: Path) -> str:
    digest = hashlib.sha1()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1 << 16), b""):
            digest.update(block)
    return digest.hexdigest()


def download(url: str, path: Path, checksum: str = "", size: int = 0) -> None:
    """`url` to `path`, whole: a temporary file is renamed in only once it checks out."""
    if path.is_file():
        if (not checksum or sha1(path) == checksum) and (not size or path.stat().st_size == size):
            return
        path.unlink()
    path.parent.mkdir(parents=True, exist_ok=True)
    for attempt in range(3):
        try:
            with urllib.request.urlopen(url, timeout=120) as response, \
                    path.with_suffix(".partial").open("wb") as out:
                while True:
                    block = response.read(1 << 16)
                    if not block:
                        break
                    out.write(block)
            partial = path.with_suffix(".partial")
            if size and partial.stat().st_size != size:
                raise ValueError(f"size {partial.stat().st_size}, expected {size}")
            if checksum and sha1(partial) != checksum:
                raise ValueError(f"sha1 {sha1(partial)}, expected {checksum}")
            partial.replace(path)
            return
        except Exception as error:  # noqa: BLE001 - retried, then reported with the URL
            path.with_suffix(".partial").unlink(missing_ok=True)
            if attempt == 2:
                raise RuntimeError(f"{url}: {error}") from error
    raise AssertionError("unreachable")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, required=True, help="the official launcher's layout")
    parser.add_argument("--lock", type=Path, default=None,
                        help="research-lock.json (default: the repository's, beside this script)")
    args = parser.parse_args()
    lock_path = args.lock or Path(__file__).resolve().parent.parent.parent / "research-lock.json"
    lock = json.loads(lock_path.read_text())["minecraft"]
    version = lock["version"]
    accepted = {lock["manifest_sha1"], *lock.get("accepted_manifest_sha1", [])}

    with urllib.request.urlopen(MANIFEST, timeout=120) as response:
        catalog = json.load(response)
    entry = next((v for v in catalog["versions"] if v["id"] == version), None)
    if entry is None:
        print(f"Mojang's manifest lists no {version}", file=sys.stderr)
        return 1
    if entry["sha1"] not in accepted:
        print(f"{version}'s version JSON ({entry['sha1']}) is not one research-lock.json accepts",
              file=sys.stderr)
        return 1

    manifest_path = args.out / "versions" / version / f"{version}.json"
    download(entry["url"], manifest_path, entry["sha1"])
    manifest = json.loads(manifest_path.read_text())
    for side, pin in (("client", "client_sha1"), ("server", "server_sha1")):
        listed = manifest["downloads"][side]["sha1"]
        if listed != lock[pin]:
            print(f"{version}'s {side} jar ({listed}) is not the pinned {lock[pin]}",
                  file=sys.stderr)
            return 1

    client = manifest["downloads"]["client"]
    server = manifest["downloads"]["server"]
    download(client["url"], args.out / "versions" / version / f"{version}.jar", client["sha1"])
    download(server["url"], args.out / f"server-{version}.jar", server["sha1"])

    libraries = [l["downloads"]["artifact"] for l in manifest["libraries"]
                 if l.get("downloads", {}).get("artifact")]
    for artifact in libraries:
        download(artifact["url"], args.out / "libraries" / artifact["path"], artifact["sha1"])

    index = manifest["assetIndex"]
    if index["sha1"] not in set(lock.get("accepted_asset_index_sha1", [])):
        print(f"{version}'s asset index ({index['sha1']}) is not one research-lock.json accepts",
              file=sys.stderr)
        return 1
    index_path = args.out / "assets" / "indexes" / f"{index['id']}.json"
    download(index["url"], index_path, index["sha1"])
    objects = json.loads(index_path.read_text())["objects"]
    files = [(f"{OBJECTS}/{checksum[:2]}/{checksum}",
              args.out / "assets" / "objects" / checksum[:2] / checksum,
              checksum, details["size"])
             for name, details in objects.items() for checksum in [details["hash"]]]
    with concurrent.futures.ThreadPoolExecutor(max_workers=JOBS) as pool:
        for future in concurrent.futures.as_completed(
            pool.submit(download, url, path, checksum, size)
            for url, path, checksum, size in files
        ):
            future.result()

    print(f"Minecraft {version}: {len(libraries)} libraries and {len(files)} asset objects "
          f"from Mojang, checked against research-lock.json")
    return 0


if __name__ == "__main__":
    sys.exit(main())
