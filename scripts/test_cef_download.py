"""Pinned release installation must reject corruption and archive traversal."""

import hashlib
from pathlib import Path
import tarfile
import tempfile
import unittest
import io

from fetch_engine import unpack

class DownloadTests(unittest.TestCase):
    def test_linux_engine_installs_and_reuses_a_target_specific_cache(self):
        import json
        import subprocess
        import sys
        from cef_artifact import platform_for, provenance_digest
        from test_cef_artifact import ArtifactTests

        for target in ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                engine = root / platform_for(target)[1]
                engine.mkdir()
                ArtifactTests().fixture(engine, target)
                manifest = json.loads((engine / "superfik-engine.json").read_text())
                archive = root / "engine.tar.gz"
                with tarfile.open(archive, "w:gz") as output:
                    output.add(engine, arcname=engine.name)
                pin = {"schema": 1, "target": target, "repository": "example/engine",
                       "tag": "test-linux", "asset": "engine.tar.gz",
                       "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
                       "provenance_sha256": provenance_digest(manifest)}
                lock = root / "release.lock.json"
                lock.write_text(json.dumps(pin))
                command = [sys.executable, str(Path(__file__).with_name("fetch_engine.py")),
                           "--lock", str(lock), "--archive", str(archive),
                           "--output", str(root / "installed"), "--target", target]
                for _ in range(2):
                    result = subprocess.run(command, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stdout.strip(), str((root / "installed").resolve()))
                result = subprocess.run(command[:-1] + ["aarch64-apple-darwin"],
                                        capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("target", result.stderr)

    def test_corrupt_archive_is_rejected_before_creating_destination(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            archive = root / "engine.tar.gz"
            archive.write_bytes(b"wrong bytes")
            with self.assertRaisesRegex(ValueError, "checksum"):
                unpack(archive, root / "engine", "0" * 64)
            self.assertFalse((root / "engine").exists())

    def test_archive_cannot_write_through_symlinks_or_escape_its_root(self):
        for names in [["../outside"],
                      ["superfik-cef-macos-arm64/link", "superfik-cef-macos-arm64/link/file"],
                      ["superfik-cef-macos-arm64/./link", "superfik-cef-macos-arm64/link/file"],
                      ["superfik-cef-macos-arm64/Link", "superfik-cef-macos-arm64/link/file"]]:
            with tempfile.TemporaryDirectory() as root:
                root = Path(root)
                archive = root / "engine.tar.gz"
                with tarfile.open(archive, "w:gz") as output:
                    for name in names:
                        entry = tarfile.TarInfo(name)
                        if name.lower().endswith("/link"):
                            entry.type = tarfile.SYMTYPE
                            entry.linkname = "."
                            output.addfile(entry)
                        else:
                            entry.size = 1
                            output.addfile(entry, io.BytesIO(b"x"))
                checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
                with self.assertRaisesRegex(ValueError, "archive"):
                    unpack(archive, root / "engine", checksum)
                self.assertFalse((root / "engine").exists())

    def test_install_and_cache_use_selected_release_pin_without_engine_sources(self):
        import json
        import subprocess
        import sys
        from cef_artifact import FRAMEWORK, provenance_digest
        from test_cef_artifact import ArtifactTests

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine = root / "superfik-cef-macos-arm64"
            engine.mkdir()
            ArtifactTests().fixture(engine)
            manifest = json.loads((engine / "superfik-engine.json").read_text())
            archive = root / "engine.tar.gz"
            with tarfile.open(archive, "w:gz") as output:
                output.add(engine, arcname=engine.name)
            pin = {
                "schema": 1, "repository": "example/engine", "tag": "test-release",
                "asset": "engine.tar.gz",
                "sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
                "provenance_sha256": provenance_digest(manifest),
            }
            lock = root / "release.lock.json"
            lock.write_text(json.dumps(pin))
            destination = root / "installed"
            command = [sys.executable, str(Path(__file__).with_name("fetch_engine.py")),
                       "--lock", str(lock), "--archive", str(archive),
                       "--output", str(destination)]
            for _ in range(2):
                result = subprocess.run(command, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), str(destination.resolve()))
            self.assertEqual((root / "installed.sha256").read_text().strip(), pin["sha256"])
            result = subprocess.run(
                [sys.executable, str(Path(__file__).with_name("cef_artifact.py")),
                 str(destination), "--lock", str(lock)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            pin["provenance_sha256"] = "0" * 64
            lock.write_text(json.dumps(pin))
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("provenance", result.stderr)
            pin["provenance_sha256"] = provenance_digest(manifest)
            lock.write_text(json.dumps(pin))
            (destination / FRAMEWORK / "Chromium Embedded Framework").write_bytes(b"changed")
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("checksums", result.stderr)

    def test_release_pin_requires_provenance(self):
        import json
        from cef_artifact import load_release_pin

        with tempfile.TemporaryDirectory() as temporary:
            lock = Path(temporary) / "release.lock.json"
            lock.write_text(json.dumps({"schema": 1, "sha256": "a" * 64}))
            with self.assertRaisesRegex(ValueError, "provenance_sha256"):
                load_release_pin(lock)
