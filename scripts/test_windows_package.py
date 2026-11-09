"""Windows packaging rejects incomplete engines and binaries that do not match the engine."""

import hashlib
import json
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest

from cef_artifact import provenance_digest, digest
from fetch_engine import unpack
from package_windows import TARGET, DLLS, RESOURCES, check_executable, stage
import test_cef_artifact

def executable(path, dll=b"libcef.dll", subsystem=2):
    data = bytearray(1024)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3c, 0x80)
    data[0x80:0x84] = b"PE\0\0"
    struct.pack_into("<HH", data, 0x84, 0x8664, 1)
    struct.pack_into("<H", data, 0x94, 240)
    optional = 0x98
    struct.pack_into("<H", data, optional, 0x20b)
    struct.pack_into("<H", data, optional + 68, subsystem)
    struct.pack_into("<II", data, optional + 120, 0x1000, 40)
    struct.pack_into("<IIII", data, optional + 240 + 8, 512, 0x1000, 512, 512)
    struct.pack_into("<IIIII", data, 512, 0, 0, 0, 0x1040, 0)
    data[576:576 + len(dll) + 1] = dll + b"\0"
    path.write_bytes(data)

class WindowsPackageTests(unittest.TestCase):
    def test_only_x64_windowed_cef_executables_can_ship(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "superfik.exe"
            executable(binary)
            check_executable(binary)
            executable(binary, subsystem=3)
            with self.assertRaisesRegex(ValueError, "GUI"):
                check_executable(binary)
            executable(binary, dll=b"kernel32.dll")
            with self.assertRaisesRegex(ValueError, "no CEF integration"):
                check_executable(binary)
            check_executable(binary, browser=False)
            executable(binary)
            with self.assertRaisesRegex(ValueError, "needs CEF"):
                check_executable(binary, browser=False)
            binary.write_bytes(b"MZ")
            with self.assertRaisesRegex(ValueError, "truncated"):
                check_executable(binary)

    def fixture(self, root):
        engine = root / "engine"
        engine.mkdir()
        test_cef_artifact.ArtifactTests().fixture(engine, TARGET)
        for name in (*DLLS, "v8_context_snapshot.bin", "vk_swiftshader_icd.json", "dxcompiler.dll"):
            file = engine / name
            file.write_bytes(b"test runtime")
        for name in (*RESOURCES, "locales/en-US.pak", "locales/pt-BR.pak"):
            file = engine / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(b"test resource")
        self.refresh(engine)
        manifest = json.loads((engine / "superfik-engine.json").read_text())
        lock = root / "release.lock.json"
        lock.write_text(json.dumps({"schema": 1, "target": TARGET, "repository": "test/engine",
                                    "tag": "test", "asset": "engine.tar.gz", "sha256": "a" * 64,
                                    "provenance_sha256": provenance_digest(manifest)}))
        binary = root / "superfik.exe"
        executable(binary)
        (root / "THIRD_PARTY_NOTICES.md").write_text("test notices")
        crt = root / "crt"
        crt.mkdir()
        for name in ("vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll", "msvcp140_1.dll"):
            (crt / name).write_bytes(b"test Visual C++ runtime")
        return engine, binary, lock, crt

    def refresh(self, engine):
        file = engine / "superfik-engine.json"
        data = json.loads(file.read_text())
        data["files"] = {p.relative_to(engine).as_posix(): digest(p)
                         for p in engine.rglob("*") if p.is_file() and p != file}
        file.write_text(json.dumps(data))

    def test_payload_includes_resources_locales_and_application_dlls(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine, binary, lock, crt = self.fixture(root)
            (root / "onnxruntime.dll").write_bytes(b"application dependency")
            (root / "dxcompiler.dll").write_bytes(b"stale engine DLL")
            payload = stage(binary, engine, root / "payload", lock, crt)
            self.assertEqual((payload / "dxcompiler.dll").read_bytes(), b"test runtime")
            for name in ("superfik.exe", "libcef.dll", "dxcompiler.dll", "onnxruntime.dll",
                         "locales/pt-BR.pak", "LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md", "superfik-engine.json",
                         "vcruntime140.dll", "msvcp140.dll"):
                self.assertTrue((payload / name).is_file(), name)
            with self.assertRaisesRegex(ValueError, "already exists"):
                stage(binary, engine, payload, lock, crt)

    def test_without_a_published_engine_the_app_ships_without_the_browser(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, binary, lock, crt = self.fixture(root)
            executable(binary, dll=b"kernel32.dll")
            payload = stage(binary, None, root / "payload", None, crt)
            for name in ("superfik.exe", "LICENSE", "NOTICE", "THIRD_PARTY_NOTICES.md", "vcruntime140.dll"):
                self.assertTrue((payload / name).is_file(), name)
            self.assertFalse((payload / "libcef.dll").exists())
            self.assertFalse((payload / "locales").exists())
            with self.assertRaisesRegex(ValueError, "both the engine and its pin"):
                stage(binary, None, root / "other", lock, crt)

    def test_missing_runtime_does_not_leave_a_partial_payload(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine, binary, lock, crt = self.fixture(root)
            (engine / "icudtl.dat").unlink()
            self.refresh(engine)
            with self.assertRaisesRegex(ValueError, "required Windows runtime"):
                stage(binary, engine, root / "payload", lock, crt)
            self.assertFalse((root / "payload").exists())

    def test_clean_install_requires_the_visual_cpp_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine, binary, lock, crt = self.fixture(root)
            (crt / "msvcp140_1.dll").unlink()
            with self.assertRaisesRegex(ValueError, "Visual C\\+\\+ x64"):
                stage(binary, engine, root / "payload", lock, crt)
            self.assertFalse((root / "payload").exists())

    def test_windows_archives_reject_ntfs_special_paths(self):
        import io
        prefix = "superfik-cef-windows-x86_64/"
        for name in ("file:stream", "NUL.txt", "file.", "directory\\outside", "COM1"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                archive = root / "engine.tar.gz"
                with tarfile.open(archive, "w:gz") as output:
                    entry = tarfile.TarInfo(prefix + name)
                    entry.size = 1
                    output.addfile(entry, io.BytesIO(b"x"))
                with self.assertRaisesRegex(ValueError, "unsafe Windows"):
                    unpack(archive, root / "output", hashlib.sha256(archive.read_bytes()).hexdigest(), target=TARGET)

if __name__ == "__main__":
    unittest.main()
