"""Explicit, resumable developer download of the pinned native CPU model pair.

Streams 64 KiB at a time, checks pinned size/SHA-256 and never overwrites a
completed file. No user recordings, text, credentials or project data are sent.
Model files retain their source terms; downloading is not model certification.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
CATALOG = ROOT / "native/desktop/models/omnivoice-q4.json"


class HTTPSRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, url):
        if not url.lower().startswith("https://"):
            raise RuntimeError("Refusing a model-download redirect outside HTTPS.")
        return super().redirect_request(request, response, code, message, headers, url)


def verify(path, specification):
    if path.stat().st_size != specification["bytes"]:
        raise RuntimeError("Existing model size differs from its pinned catalog; file preserved.")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(65536), b""):
            digest.update(block)
    if digest.hexdigest() != specification["sha256"]:
        raise RuntimeError("Model checksum differs from its pinned catalog; file preserved.")


def download(destination, catalog, rate_mib=4.0):
    if not destination.is_absolute():
        raise ValueError("Choose an absolute model destination.")
    destination.mkdir(parents=True, exist_ok=True)
    # Optional Windows certificate-store integration; never disables TLS checks.
    try:
        import truststore
        truststore.inject_into_ssl()
    except ImportError:
        pass
    opener = urllib.request.build_opener(HTTPSRedirects())
    for specification in catalog["files"]:
        name = specification["name"]
        if not re.fullmatch(r"[A-Za-z0-9_.-]+\.gguf", name):
            raise ValueError("Unsafe model catalog filename.")
        target = destination / name
        if target.exists():
            verify(target, specification)
            print(f"Verified existing {name}", flush=True)
            continue
        part = destination / (name + ".part")
        metadata = destination / (name + ".part.json")
        identity = {"repository": catalog["repository"], "revision": catalog["revision"], **specification}
        if metadata.exists():
            if json.loads(metadata.read_text(encoding="utf-8")) != identity:
                raise RuntimeError("Partial download belongs to a different catalog; preserved.")
        elif part.exists():
            raise RuntimeError("Unidentified partial download exists; preserved.")
        else:
            with metadata.open("x", encoding="utf-8") as stream:
                json.dump(identity, stream)
        offset = part.stat().st_size if part.exists() else 0
        if offset > specification["bytes"]:
            raise RuntimeError("Partial model exceeds its pinned size; preserved.")
        if shutil.disk_usage(destination).free < specification["bytes"] - offset + 128 * 1024 * 1024:
            raise RuntimeError("Insufficient free disk space for the selected model file.")
        if offset < specification["bytes"]:
            url = f"https://huggingface.co/{catalog['repository']}/resolve/{catalog['revision']}/{name}"
            headers = {"Accept-Encoding": "identity", "User-Agent": "VoiceStudioNative-DeveloperFetch/1"}
            if offset:
                headers["Range"] = f"bytes={offset}-"
            request = urllib.request.Request(url, headers=headers)
            with opener.open(request, timeout=30) as response:
                if offset and (response.status != 206 or not response.headers.get("Content-Range", "").startswith(f"bytes {offset}-")):
                    raise RuntimeError("Server did not honor resume; existing partial file preserved.")
                length = response.headers.get("Content-Length")
                if length is not None and int(length) != specification["bytes"] - offset:
                    raise RuntimeError("Model response length differs from the pinned catalog.")
                print(f"Downloading {name}, starting at {offset} bytes (limit {rate_mib:g} MiB/s)", flush=True)
                started = time.monotonic()
                received = 0
                checkpoint = offset
                with part.open("ab" if offset else "xb") as output:
                    while True:
                        block = response.read(65536)
                        if not block:
                            break
                        if offset + received + len(block) > specification["bytes"]:
                            raise RuntimeError("Model response exceeded its pinned size.")
                        output.write(block)
                        received += len(block)
                        delay = received / (rate_mib * 1024 * 1024) - (time.monotonic() - started)
                        if delay > 0:
                            time.sleep(min(delay, 0.1))
                        if offset + received - checkpoint >= 64 * 1024 * 1024:
                            checkpoint = offset + received
                            print(f"  {name}: {checkpoint}/{specification['bytes']} bytes", flush=True)
                    output.flush()
                    os.fsync(output.fileno())
        verify(part, specification)
        # Windows rename refuses an existing destination. POSIX link is atomic
        # and non-overwriting, unlike POSIX rename.
        if os.name == "nt":
            part.rename(target)
        else:
            os.link(part, target)
            part.unlink()
        metadata.unlink()
        print(f"Verified and published {name}", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", required=True, type=Path)
    parser.add_argument("--rate-mib", type=float, default=4.0)
    arguments = parser.parse_args()
    if not 0.25 <= arguments.rate_mib <= 16:
        parser.error("--rate-mib must be between 0.25 and 16")
    download(arguments.destination, json.loads(CATALOG.read_text(encoding="utf-8")), arguments.rate_mib)
