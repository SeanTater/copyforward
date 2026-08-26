from pathlib import Path

import yaml


ROOT = Path(__file__).parents[1]


def test_python_metadata_advertises_current_supported_versions():
    metadata = (ROOT / "pyproject.toml").read_text()

    assert 'requires-python = ">=3.10"' in metadata
    for version in ("3.10", "3.11", "3.12", "3.13", "3.14"):
        assert f'Programming Language :: Python :: {version}' in metadata


def test_wheel_matrix_covers_requested_platforms_and_python_versions():
    workflow = yaml.safe_load((ROOT / ".github/workflows/python-ci.yml").read_text())
    matrix = workflow["jobs"]["build-wheels"]["strategy"]["matrix"]
    platforms = {platform["target"] for platform in matrix["platform"]}

    assert {"x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"} <= platforms
    assert "aarch64-apple-darwin" in platforms
    assert "3.14" in matrix["python-version"]


def test_python_feature_has_no_unused_cffi_native_extension():
    cargo_toml = (ROOT / "Cargo.toml").read_text()

    assert "cffi =" not in cargo_toml
    assert 'python = ["dep:pyo3", "dep:numpy"]' in cargo_toml
