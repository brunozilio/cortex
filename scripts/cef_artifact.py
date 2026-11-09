"""Validate a custom CEF distribution before Cargo can select/download CEF.

The manifest is provenance, not a signature. Only use artifacts from a trusted
builder. The framework must be validated before codesigning changes its bytes.
"""

import argparse
import hashlib
import json
import re
from pathlib import Path

CEF_COMMIT = "14c5a089e8452874cb1a556dc3fde26ed2d87723"
CEF_VERSION = "154.0.34+g14c5a08+chromium-154.0.8037.98"
TARGET = "aarch64-apple-darwin"
FRAMEWORK = "Chromium Embedded Framework.framework"
ENGINE_VERSION_ENTRY = 0x535546
ENGINE_ABI = 1
ROOT = Path(__file__).resolve().parent.parent
PLATFORMS = {
    TARGET: ("macosarm64", "superfik-cef-macos-arm64", f"{FRAMEWORK}/Chromium Embedded Framework"),
    "x86_64-unknown-linux-gnu": ("linux64", "superfik-cef-linux-x86_64", "libcef.so"),
    "aarch64-unknown-linux-gnu": ("linuxarm64", "superfik-cef-linux-aarch64", "libcef.so"),
    "x86_64-pc-windows-msvc": ("windows64", "superfik-cef-windows-x86_64", "libcef.dll"),
}

def platform_for(target):
    if target not in PLATFORMS:
        raise ValueError(f"unsupported engine target: {target}")
    return PLATFORMS[target]

def load_release_pin(path=ROOT / "scripts/cef-release.lock.json"):
    pin = json.loads(path.read_text())
    if pin.get("schema") != 1:
        raise ValueError("invalid engine release pin")
    platform_for(pin.get("target", TARGET))
    for key in ("sha256", "provenance_sha256"):
        if not re.fullmatch(r"[0-9a-f]{64}", pin.get(key, "")):
            raise ValueError(f"invalid engine release {key}")
    for key in ("tag", "asset"):
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", pin.get(key, "")):
            raise ValueError("invalid release tag or asset name")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*", pin.get("repository", "")):
        raise ValueError("invalid engine repository")
    return pin

def provenance_digest(manifest):
    """Hash the engine's ordered patches and build configuration as canonical JSON."""
    provenance = {key: manifest.get(key) for key in ("patches", "build")}
    return hashlib.sha256(json.dumps(
        provenance, sort_keys=True, separators=(",", ":")
    ).encode()).hexdigest()

def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()

def validate(directory, expected_provenance=None, target=TARGET):
    platform, _, binary = platform_for(target)
    directory = directory.resolve(strict=True)
    manifest = directory / "superfik-engine.json"
    if not manifest.is_file():
        raise ValueError("custom engine manifest is missing")
    data = json.loads(manifest.read_text())
    if (data.get("schema"), data.get("cef_commit"), data.get("target")) != (
        1, CEF_COMMIT, target
    ):
        raise ValueError("unsupported engine schema, CEF revision or target")
    patches = data.get("patches")
    if not isinstance(patches, list) or not patches:
        raise ValueError("custom engine must identify its patches")
    if expected_provenance is not None and provenance_digest(data) != expected_provenance:
        raise ValueError("artifact provenance differs from release pin")
    for patch in patches:
        if not isinstance(patch, dict) or not patch.get("name"):
            raise ValueError("invalid patch identity")
        checksum = patch.get("sha256", "")
        if len(checksum) != 64 or any(c not in "0123456789abcdef" for c in checksum):
            raise ValueError("invalid patch checksum")
    files = data.get("files")
    if not isinstance(files, dict) or not files:
        raise ValueError("artifact file checksums are missing")
    actual = {}
    links = {}
    for path in directory.rglob("*"):
        if not path.resolve().is_relative_to(directory):
            raise ValueError("artifact symlink escapes distribution")
        if path.is_symlink():
            links[path.relative_to(directory).as_posix()] = path.readlink().as_posix()
        if path.is_file() and path != manifest:
            actual[path.relative_to(directory).as_posix()] = digest(path)
    if actual != files:
        raise ValueError("artifact contents do not match manifest checksums")
    if links != data.get("symlinks", {}):
        raise ValueError("artifact symlinks do not match manifest")
    header = directory / "include/cef_version.h"
    if f'#define CEF_VERSION "{CEF_VERSION}"' not in header.read_text():
        raise ValueError("CEF header version does not match pinned ABI")
    if CEF_COMMIT not in header.read_text():
        raise ValueError("CEF header commit does not match pinned revision")
    archive = json.loads((directory / "archive.json").read_text())
    names = {f"cef_binary_{CEF_VERSION}_{platform}_minimal{extension}"
             for extension in (".zip", ".tar.bz2")}
    if archive.get("name") not in names:
        raise ValueError("archive metadata does not match pinned ABI")
    if archive.get("type") != "minimal" or not re.fullmatch("[0-9a-f]{40}", archive.get("sha1", "")):
        raise ValueError("archive metadata is incompatible with cef-dll-sys")
    binary = directory / binary
    if not binary.is_file():
        raise ValueError("CEF framework binary is missing")
    return directory

def probe(directory, target=TARGET):
    """Verify the compiled native marker without launching a browser/profile."""
    import ctypes
    binary = directory / platform_for(target)[2]
    library = ctypes.CDLL(str(binary))
    version = library.cef_version_info
    version.argtypes = [ctypes.c_int]
    version.restype = ctypes.c_int
    if version(ENGINE_VERSION_ENTRY) != ENGINE_ABI:
        raise ValueError("framework is missing the Superfik native engine marker")

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("distribution", type=Path)
    parser.add_argument("--lock", type=Path, default=ROOT / "scripts/cef-release.lock.json")
    parser.add_argument("--probe", action="store_true")
    parser.add_argument("--target", choices=PLATFORMS)
    args = parser.parse_args()
    pin = load_release_pin(args.lock)
    target = pin.get("target", TARGET)
    if args.target and args.target != target:
        raise ValueError("engine release pin does not match requested target")
    directory = validate(args.distribution, pin["provenance_sha256"], target)
    if args.probe:
        probe(directory, target)
    print(directory)

if __name__ == "__main__":
    main()
