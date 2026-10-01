"""Shared, dependency-free GPUI release naming, staging and provenance checks."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPO = ROOT.parent
TARGETS = {
    ("windows", "x86_64"): "zip",
    ("windows", "arm64"): "zip",
    ("linux", "x86_64"): "AppImage",
    ("linux", "arm64"): "AppImage",
    ("macos", "arm64"): "dmg",
}


def version(value: str) -> str:
    if not re.fullmatch(
        r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)", value
    ):
        raise ValueError("Expected a stable X.Y.Z version")
    return value


def filename(value: str, platform: str, arch: str) -> str:
    return f"EasyCommandRunner-GPUI-v{version(value)}-{platform}-{arch}.{TARGETS[platform, arch]}"


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def command_output(command: list[str], *, cwd: Path = ROOT) -> str:
    """Cargo emits UTF-8 JSON regardless of the Windows process locale."""
    return subprocess.check_output(command, cwd=cwd, encoding="utf-8")


def prepare(
    value: str, platform: str, arch: str, binary: Path, output: Path, sha: str
) -> None:
    filename(value, platform, arch)
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("A full immutable Git commit SHA is required")
    if output.exists():
        raise ValueError(
            f"Staging directory already exists; refusing to overwrite: {output}"
        )
    if not binary.is_file():
        raise ValueError(f"Release binary does not exist: {binary}")
    package_version = version(
        tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"][
            "version"
        ]
    )
    if value != package_version:
        raise ValueError("Package version does not match Cargo.toml")
    actual_sha = command_output(["git", "rev-parse", "HEAD"]).strip()
    if actual_sha != sha:
        raise ValueError("SOURCE_SHA does not match the checked-out source")
    output.mkdir(parents=True)
    shutil.copy2(REPO / "LICENSE", output / "LICENSE")
    licenses = output / "licenses"
    licenses.mkdir()
    for source, target in (
        ("LICENSE-LUCIDE", "LICENSE-LUCIDE"),
        ("LICENSE", "LICENSE-DEVICON"),
    ):
        shutil.copy2(ROOT / "assets/tab-icons" / source, licenses / target)
    shutil.copy2(ROOT / "packaging/README-runtime.md", output / "README-runtime.md")
    rust_info = command_output(["rustc", "-vV"])
    host_match = re.search(r"^host: (.+)$", rust_info, re.MULTILINE)
    if host_match is None:
        raise ValueError("Cannot determine the native Rust target")
    host = host_match.group(1)
    dependencies = json.loads(
        command_output(
            [
                "cargo",
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
                "--filter-platform",
                host,
            ],
        )
    )["packages"]
    notices = []
    for package in dependencies:
        notices.append(
            {
                key: package.get(key)
                for key in ("name", "version", "license", "repository")
            }
        )
        parent = Path(package["manifest_path"]).parent
        candidates = [
            p
            for p in parent.iterdir()
            if p.is_file()
            and p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE"))
        ]
        if package.get("license_file"):
            candidates.append(parent / package["license_file"])
        for path in set(candidates):
            if path.is_file():
                dest = licenses / f"{package['name']}-{package['version']}"
                dest.mkdir(exist_ok=True)
                shutil.copy2(path, dest / path.name)
    (output / "dependency-notices.json").write_text(
        json.dumps(notices, indent=2) + "\n", encoding="utf-8"
    )
    dirty = bool(command_output(["git", "status", "--porcelain"], cwd=REPO).strip())
    metadata = {
        "version": value,
        "platform": platform,
        "arch": arch,
        "source_sha": sha,
        "source_tree_clean": not dirty,
        "input_binary_sha256": digest(binary),
        "profile": "release",
        "publisher_signed": False,
        "code_signing": "ad-hoc" if platform == "macos" else "unsigned",
    }
    (output / "build-info.json").write_text(
        json.dumps(metadata, indent=2) + "\n", encoding="utf-8"
    )


def verify(directory: Path, value: str, sha: str) -> None:
    expected = {filename(value, p, a) for p, a in TARGETS}
    packages = {
        p.name for p in directory.iterdir() if p.suffix in {".zip", ".dmg", ".AppImage"}
    }
    if packages != expected:
        raise ValueError(
            f"Package set mismatch: missing={expected - packages}, extra={packages - expected}"
        )
    lines = []
    for platform, arch in TARGETS:
        name = filename(value, platform, arch)
        package = directory / name
        info = json.loads(
            (directory / (name + ".build-info.json")).read_text(encoding="utf-8")
        )
        if info.get("source_tree_clean") is not True:
            raise ValueError(f"Uncommitted source cannot be published: {name}")
        if (
            info["version"],
            info["platform"],
            info["arch"],
            info["source_sha"],
            info["profile"],
        ) != (value, platform, arch, sha, "release"):
            raise ValueError(f"Incorrect build provenance: {name}")
        if package.stat().st_size < 1024 or digest(package) != info["package_sha256"]:
            raise ValueError(f"Package corrupt or checksum mismatch: {name}")
        lines.append(f"{info['package_sha256']}  {name}\n")
    (directory / "SHA256SUMS.txt").write_text("".join(lines), encoding="utf-8")


def verify_upload(directory: Path, value: str, sha: str, release: dict) -> None:
    if release.get("isDraft") is not True or release.get("targetCommitish") != sha:
        raise ValueError(
            "Upload must belong to an unpublished draft pinned to the source commit"
        )
    packages = {filename(value, platform, arch) for platform, arch in TARGETS}
    expected = (
        packages | {name + ".build-info.json" for name in packages} | {"SHA256SUMS.txt"}
    )
    files = {path.name: path for path in directory.iterdir() if path.is_file()}
    assets = release.get("assets", [])
    if (
        set(files) != expected
        or {asset["name"] for asset in assets} != expected
        or len(assets) != len(expected)
    ):
        raise ValueError(
            "Draft assets must match the complete five-package set, with no stale extras"
        )
    for asset in assets:
        path = files[asset["name"]]
        if asset["size"] != path.stat().st_size:
            raise ValueError(f"Uploaded asset size mismatch: {asset['name']}")
        if asset.get("digest") and asset["digest"] != "sha256:" + digest(path):
            raise ValueError(f"Uploaded asset hash mismatch: {asset['name']}")


def main() -> None:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("version")
    name = sub.add_parser("name")
    stage = sub.add_parser("prepare")
    stamp = sub.add_parser("stamp")
    check = sub.add_parser("verify")
    upload = sub.add_parser("verify-upload")
    for command in (name, stage, stamp, check, upload):
        command.add_argument("--version", required=True)
    for command in (name, stage):
        command.add_argument(
            "--platform", choices=("windows", "linux", "macos"), required=True
        )
        command.add_argument("--arch", choices=("x86_64", "arm64"), required=True)
    stage.add_argument("--binary", type=Path, required=True)
    stage.add_argument("--output", type=Path, required=True)
    stage.add_argument("--source-sha", required=True)
    stamp.add_argument("--package", type=Path, required=True)
    stamp.add_argument("--info", type=Path, required=True)
    for command in (check, upload):
        command.add_argument("--directory", type=Path, required=True)
        command.add_argument("--source-sha", required=True)
    upload.add_argument("--release-json", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "version":
        print(
            version(
                tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))[
                    "package"
                ]["version"]
            )
        )
    elif args.command == "name":
        print(filename(args.version, args.platform, args.arch))
    elif args.command == "prepare":
        prepare(
            args.version,
            args.platform,
            args.arch,
            args.binary,
            args.output,
            args.source_sha,
        )
    elif args.command == "stamp":
        info = json.loads(args.info.read_text(encoding="utf-8"))
        if info["version"] != version(args.version):
            raise ValueError("Staged version mismatch")
        if args.package.name != filename(args.version, info["platform"], info["arch"]):
            raise ValueError("Incorrect package name")
        info["package_sha256"] = digest(args.package)
        args.package.with_name(args.package.name + ".build-info.json").write_text(
            json.dumps(info, indent=2) + "\n", encoding="utf-8"
        )
    elif args.command == "verify":
        verify(args.directory, args.version, args.source_sha)
    else:
        verify_upload(
            args.directory,
            args.version,
            args.source_sha,
            json.loads(args.release_json.read_text(encoding="utf-8")),
        )


if __name__ == "__main__":
    main()
