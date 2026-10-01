"""Offline release contract tests; no GitHub mutation or GUI interaction."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import release_support as release


class ReleaseContractTests(unittest.TestCase):
    def test_utf8_command_output_ignores_legacy_windows_locale(self) -> None:
        payload = json.dumps(
            {"packages": [{"description": "中文图标 Ł 🌟"}]}, ensure_ascii=False
        ).encode("utf-8")
        with self.assertRaises(UnicodeDecodeError):
            payload.decode("cp1252")  # Reproduce the exact runner encoding failure.
        original = subprocess.check_output
        with (
            patch("locale.getencoding", return_value="cp1252"),
            patch.object(release.subprocess, "check_output", wraps=original) as checked,
        ):
            text = release.command_output(
                [
                    sys.executable,
                    "-c",
                    f"import sys;sys.stdout.buffer.write({payload!r})",
                ]
            )
            self.assertEqual(
                json.loads(text)["packages"][0]["description"], "中文图标 Ł 🌟"
            )
            self.assertEqual(checked.call_args.kwargs["encoding"], "utf-8")

    @unittest.skipIf(os.name == "nt", "Help stream probe needs a POSIX shell")
    def test_linux_help_probe_accepts_stderr_and_still_rejects_missing_options(
        self,
    ) -> None:
        script = (release.ROOT / "scripts/package-linux.sh").read_text(encoding="utf-8")
        start = script.index('"$work/linuxdeploy" --help')
        end = script.index('cp "$work/linuxdeploy-release.json"', start)
        probe = "set -euo pipefail\n" + script[start:end]
        for stream in ("stdout", "stderr"):
            for complete in (True, False):
                with (
                    self.subTest(stream=stream, complete=complete),
                    tempfile.TemporaryDirectory() as temp,
                ):
                    work = Path(temp)
                    options = "--library --executable --icon-filename --custom-apprun"
                    if complete:
                        options += " --output"
                    redirect = " >&2" if stream == "stderr" else ""
                    fake = work / "linuxdeploy"
                    fake.write_text(
                        f"#!/usr/bin/env bash\nprintf '%s\\n' '{options}'{redirect}\n",
                        encoding="utf-8",
                    )
                    fake.chmod(0o755)
                    result = subprocess.run(
                        ["bash", "-c", probe],
                        env={**os.environ, "work": str(work)},
                        capture_output=True,
                        encoding="utf-8",
                        check=False,
                    )
                    self.assertEqual(result.returncode == 0, complete, result.stderr)

    def test_names_cover_exactly_five_native_packages(self) -> None:
        self.assertEqual(len(release.TARGETS), 5)
        names = {release.filename("1.0.0", p, a) for p, a in release.TARGETS}
        self.assertEqual(len(names), 5)
        self.assertIn("EasyCommandRunner-GPUI-v1.0.0-windows-arm64.zip", names)
        self.assertIn("EasyCommandRunner-GPUI-v1.0.0-linux-x86_64.AppImage", names)
        self.assertIn("EasyCommandRunner-GPUI-v1.0.0-macos-arm64.dmg", names)

    def test_unsafe_versions_and_unknown_architectures_are_rejected(self) -> None:
        for value in ("../1.0.0", "1.0.0;echo", "v1.0.0", "01.0.0", "1.0.0-beta", ""):
            with self.assertRaises(ValueError):
                release.filename(value, "windows", "x86_64")
        with self.assertRaises(KeyError):
            release.filename("1.0.0", "macos", "x86_64")

    def make_packages(self, folder: Path) -> None:
        for platform, arch in release.TARGETS:
            name = release.filename("1.0.0", platform, arch)
            path = folder / name
            path.write_bytes(b"release package\n" * 100)
            info = {
                "version": "1.0.0",
                "platform": platform,
                "arch": arch,
                "source_sha": "a" * 40,
                "source_tree_clean": True,
                "profile": "release",
                "package_sha256": release.digest(path),
            }
            (folder / (name + ".build-info.json")).write_text(json.dumps(info))

    def test_verification_emits_checksums_for_all_packages(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            self.make_packages(folder)
            release.verify(folder, "1.0.0", "a" * 40)
            self.assertEqual(
                len((folder / "SHA256SUMS.txt").read_text().splitlines()), 5
            )

    def test_missing_corrupt_and_wrong_source_packages_never_pass(self) -> None:
        for failure in (
            "missing",
            "corrupt",
            "wrong-sha",
            "debug",
            "extra",
            "dirty",
            "untyped-clean",
        ):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temp:
                folder = Path(temp)
                self.make_packages(folder)
                path = folder / release.filename("1.0.0", "windows", "x86_64")
                info_path = folder / (path.name + ".build-info.json")
                if failure == "missing":
                    path.unlink()
                elif failure == "corrupt":
                    path.write_bytes(b"wrong data" * 300)
                elif failure == "extra":
                    (folder / "unexpected.zip").write_bytes(b"extra")
                else:
                    info = json.loads(info_path.read_text())
                    if failure in {"dirty", "untyped-clean"}:
                        info["source_tree_clean"] = (
                            False if failure == "dirty" else "true"
                        )
                    else:
                        info["source_sha" if failure == "wrong-sha" else "profile"] = (
                            "b" * 40 if failure == "wrong-sha" else "debug"
                        )
                    info_path.write_text(json.dumps(info))
                with self.assertRaises(ValueError):
                    release.verify(folder, "1.0.0", "a" * 40)
                self.assertFalse((folder / "SHA256SUMS.txt").exists())

    def test_upload_requires_exact_assets_and_same_commit_draft(self) -> None:
        for failure in (
            None,
            "missing",
            "extra",
            "duplicate",
            "size",
            "hash",
            "published",
            "source",
        ):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temp:
                folder = Path(temp)
                self.make_packages(folder)
                release.verify(folder, "1.0.0", "a" * 40)
                assets = [
                    {"name": path.name, "size": path.stat().st_size}
                    for path in folder.iterdir()
                ]
                draft = {"isDraft": True, "targetCommitish": "a" * 40, "assets": assets}
                if failure == "missing":
                    assets.pop()
                elif failure == "extra":
                    assets.append({"name": "debug.exe", "size": 1000})
                elif failure == "duplicate":
                    assets.append(assets[0].copy())
                elif failure == "size":
                    assets[0]["size"] += 1
                elif failure == "hash":
                    assets[0]["digest"] = "sha256:" + "0" * 64
                elif failure == "published":
                    draft["isDraft"] = False
                elif failure == "source":
                    draft["targetCommitish"] = "b" * 40
                if failure is None:
                    release.verify_upload(folder, "1.0.0", "a" * 40, draft)
                else:
                    with self.assertRaises(ValueError):
                        release.verify_upload(folder, "1.0.0", "a" * 40, draft)

    def test_missing_build_info_never_emits_checksums(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            self.make_packages(folder)
            name = release.filename("1.0.0", "linux", "arm64")
            (folder / (name + ".build-info.json")).unlink()
            with self.assertRaises(FileNotFoundError):
                release.verify(folder, "1.0.0", "a" * 40)
            self.assertFalse((folder / "SHA256SUMS.txt").exists())

    def test_version_and_source_must_match_checkout_before_staging(self) -> None:
        current = release.tomllib.loads((release.ROOT / "Cargo.toml").read_text())[
            "package"
        ]["version"]
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            binary = folder / "binary"
            binary.write_bytes(b"release")
            stage = folder / "staged"
            with self.assertRaises(ValueError):
                release.prepare("999.0.0", "macos", "arm64", binary, stage, "a" * 40)
            self.assertFalse(stage.exists())
            with (
                patch.object(
                    release.subprocess, "check_output", return_value="b" * 40 + "\n"
                ),
                self.assertRaises(ValueError),
            ):
                release.prepare(current, "macos", "arm64", binary, stage, "a" * 40)
            self.assertFalse(stage.exists())

    def test_staging_refuses_overwrite_missing_binary_and_non_sha(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            binary = folder / "binary"
            binary.write_bytes(b"release")
            with self.assertRaises(ValueError):
                release.prepare("1.0.0", "windows", "x86_64", binary, folder, "a" * 40)
            with self.assertRaises(ValueError):
                release.prepare(
                    "1.0.0", "windows", "x86_64", binary, folder / "new", "gpui"
                )
            with self.assertRaises(ValueError):
                release.prepare(
                    "1.0.0",
                    "windows",
                    "x86_64",
                    folder / "missing",
                    folder / "new",
                    "a" * 40,
                )


if __name__ == "__main__":
    unittest.main()
