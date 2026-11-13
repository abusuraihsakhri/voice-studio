"""Offline tests for the explicit model fetcher; never access the network."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch, MagicMock

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("native_model_fetch", ROOT / "scripts/download-native-models.py")
FETCH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FETCH)


class Response(io.BytesIO):
    def __init__(self, data, offset=0, total=None):
        super().__init__(data)
        self.status = 206 if offset else 200
        self.headers = {"Content-Length": str(len(data))}
        if offset:
            self.headers["Content-Range"] = f"bytes {offset}-{offset+len(data)-1}/{total}"


class ModelDownloadTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="voice-studio-fetch-test-")
        self.path = Path(self.directory.name).resolve()
        self.body = b"GGUF-native-fixture"
        self.file = {"name": "fixture.gguf", "bytes": len(self.body), "sha256": hashlib.sha256(self.body).hexdigest()}
        self.catalog = {"repository": "fixture/model", "revision": "test-only", "files": [self.file]}

    def tearDown(self):
        self.directory.cleanup()

    def run_download(self, response):
        opener = MagicMock()
        opener.open.return_value = response
        with patch.object(FETCH.urllib.request, "build_opener", return_value=opener):
            FETCH.download(self.path, self.catalog, rate_mib=16)
        return opener

    def test_verified_download_publishes_complete_file(self):
        self.run_download(Response(self.body))
        self.assertEqual((self.path / "fixture.gguf").read_bytes(), self.body)
        self.assertFalse((self.path / "fixture.gguf.part").exists())

    def test_completed_file_is_not_overwritten(self):
        target = self.path / "fixture.gguf"
        target.write_bytes(b"preserve this")
        with self.assertRaises(RuntimeError):
            self.run_download(Response(self.body))
        self.assertEqual(target.read_bytes(), b"preserve this")

    def test_resume_requires_matching_identity_and_exact_range(self):
        (self.path / "fixture.gguf.part").write_bytes(self.body[:4])
        identity = {"repository": self.catalog["repository"], "revision": self.catalog["revision"], **self.file}
        (self.path / "fixture.gguf.part.json").write_text(json.dumps(identity), encoding="utf-8")
        opener = self.run_download(Response(self.body[4:], offset=4, total=len(self.body)))
        self.assertEqual(opener.open.call_args.args[0].get_header("Range"), "bytes=4-")
        self.assertEqual((self.path / "fixture.gguf").read_bytes(), self.body)

    def test_wrong_response_size_and_unsafe_names_never_publish(self):
        with self.assertRaises(RuntimeError):
            self.run_download(Response(self.body + b"extra"))
        self.assertFalse((self.path / "fixture.gguf").exists())
        self.catalog["files"][0]["name"] = "../escape.gguf"
        with self.assertRaises(ValueError):
            self.run_download(Response(self.body))


if __name__ == "__main__":
    unittest.main()
