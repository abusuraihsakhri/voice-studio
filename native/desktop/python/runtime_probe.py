"""Discover an explicitly selected runtime without importing model/GPU packages.

Embedded by the Rust client, executed with Python isolated mode in a bounded
Windows Job. This is a preflight, not proof of successful backend/model startup.
"""
import importlib.util
import json
import os
from pathlib import Path
import sys

REQUIRED_MODULES = (
    "fastapi", "uvicorn", "psutil", "multipart", "numpy", "soundfile",
    "huggingface_hub", "safetensors", "torch", "transformers", "sqlalchemy",
    "alembic", "pydantic",
)


def inspect_runtime():
    missing = []
    for name in REQUIRED_MODULES:
        try:
            present = importlib.util.find_spec(name) is not None
        except (ImportError, ValueError, AttributeError):
            present = False
        if not present:
            missing.append(name)
    version_ok = sys.version_info >= (3, 11)
    return {
        "schema_version": 1,
        "scope": "module-discovery-only",
        "python_version": ".".join(str(x) for x in sys.version_info[:3]),
        "python_version_supported": version_ok,
        "missing_modules": missing,
        "ready": version_ok and not missing,
        "model_execution_verified": False,
        "gpu_modules_imported": any(x in sys.modules for x in ("torch", "tensorflow", "cupy")),
        "device_policy": os.environ.get("OMNIVOICE_DEVICE", "unspecified"),
    }


def write_report(path):
    # Never overwrite an existing report, draft or user file.
    with Path(path).open("x", encoding="utf-8") as stream:
        json.dump(inspect_runtime(), stream, ensure_ascii=True)
        stream.flush()
        os.fsync(stream.fileno())


if __name__ == "__main__":
    write_report(sys.argv[1])
