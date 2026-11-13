"""Stdlib-only tests; do not import the project's heavyweight conftest."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[2] / "native/desktop/python/runtime_probe.py"
SPEC = importlib.util.spec_from_file_location("native_runtime_probe", SCRIPT)
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class RuntimeProbeTests(unittest.TestCase):
    def test_discovers_modules_without_importing_them(self):
        with patch.object(PROBE.importlib.util, "find_spec", return_value=object()) as discover:
            report = PROBE.inspect_runtime()
        self.assertEqual(discover.call_count, len(PROBE.REQUIRED_MODULES))
        self.assertEqual(report["missing_modules"], [])
        self.assertFalse(report["model_execution_verified"])
        self.assertFalse(report["gpu_modules_imported"])

    def test_missing_modules_and_probe_failure_are_not_ready(self):
        with patch.object(PROBE.importlib.util, "find_spec", side_effect=ImportError):
            report = PROBE.inspect_runtime()
        self.assertFalse(report["ready"])
        self.assertEqual(report["missing_modules"], list(PROBE.REQUIRED_MODULES))

    def test_report_does_not_overwrite_existing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            path.write_text("original", encoding="utf-8")
            with self.assertRaises(FileExistsError):
                PROBE.write_report(path)
            self.assertEqual(path.read_text(encoding="utf-8"), "original")


if __name__ == "__main__":
    unittest.main()
