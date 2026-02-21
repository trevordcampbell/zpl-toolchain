#!/usr/bin/env python3
"""Preflight checks for publishable workspace crates.

Checks:
1) Publishable crates must not use path-only dependencies (path without version).
2) Publishable crates must package cleanly via `cargo package --list --no-verify`.
"""

from __future__ import annotations

import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import tomllib


ROOT = Path(__file__).resolve().parents[1]
DEPENDENCY_SECTIONS = ("dependencies", "build-dependencies", "dev-dependencies")


@dataclass(frozen=True)
class Package:
    name: str
    manifest: Path
    publish: bool


def load_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def workspace_members() -> list[Path]:
    workspace = load_toml(ROOT / "Cargo.toml")
    members = workspace.get("workspace", {}).get("members", [])
    return [ROOT / member / "Cargo.toml" for member in members]


def publishable_packages() -> list[Package]:
    packages: list[Package] = []
    for manifest in workspace_members():
        if not manifest.exists():
            continue
        data = load_toml(manifest)
        package = data.get("package", {})
        name = package.get("name")
        if not name:
            continue
        publish = package.get("publish", True) is not False
        packages.append(Package(name=name, manifest=manifest, publish=publish))
    return [package for package in packages if package.publish]


def check_path_dependency_versions(packages: list[Package]) -> list[str]:
    errors: list[str] = []
    for package in packages:
        data = load_toml(package.manifest)
        for section in DEPENDENCY_SECTIONS:
            deps = data.get(section, {})
            for dep_name, spec in deps.items():
                if not isinstance(spec, dict):
                    continue
                has_path = "path" in spec
                has_version = "version" in spec
                if has_path and not has_version:
                    rel = package.manifest.relative_to(ROOT)
                    errors.append(
                        f"{rel}: {section}.{dep_name} uses path without version"
                    )
    return errors


def check_package_list(packages: list[Package]) -> list[str]:
    errors: list[str] = []
    for package in packages:
        cmd = [
            "cargo",
            "package",
            "-p",
            package.name,
            "--allow-dirty",
            "--no-verify",
            "--list",
        ]
        result = subprocess.run(
            cmd,
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            errors.append(
                f"{package.name}: cargo package preflight failed\n{result.stderr.strip()}"
            )
    return errors


def main() -> int:
    packages = publishable_packages()
    print(f"publishable crates: {len(packages)}")

    path_dep_errors = check_path_dependency_versions(packages)
    if path_dep_errors:
        print("\npath dependency policy violations:")
        for error in path_dep_errors:
            print(f"- {error}")
        return 1

    package_errors = check_package_list(packages)
    if package_errors:
        print("\ncargo package preflight failures:")
        for error in package_errors:
            print(f"- {error}")
        return 1

    print("publish preflight checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
