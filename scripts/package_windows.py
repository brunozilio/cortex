"""Stage an x64 Windows MSI payload, with a validated, pinned CEF distribution when one is published."""

import argparse
from pathlib import Path
import shutil
import struct
import tempfile

from cef_artifact import ROOT, load_release_pin, validate

TARGET = "x86_64-pc-windows-msvc"
DLLS = ("libcef.dll", "chrome_elf.dll", "libEGL.dll", "libGLESv2.dll",
        "d3dcompiler_47.dll", "vk_swiftshader.dll", "vulkan-1.dll")
RESOURCES = ("icudtl.dat", "resources.pak", "chrome_100_percent.pak", "chrome_200_percent.pak")

def check_executable(binary, browser=True):
    """Refuse console or wrong-architecture binaries, and a browser that does not match the engine."""
    data = binary.read_bytes()
    try:
        if data[:2] != b"MZ":
            raise ValueError("not a Windows executable")
        pe = struct.unpack_from("<I", data, 0x3c)[0]
        if data[pe:pe + 4] != b"PE\0\0":
            raise ValueError("invalid Windows executable signature")
        machine, count = struct.unpack_from("<HH", data, pe + 4)
        optional_size = struct.unpack_from("<H", data, pe + 20)[0]
        optional = pe + 24
        magic = struct.unpack_from("<H", data, optional)[0]
        subsystem = struct.unpack_from("<H", data, optional + 68)[0]
        if (machine, magic, subsystem) != (0x8664, 0x20b, 2):
            raise ValueError("Windows release must be an x64 GUI executable")
        if optional_size < 128:
            raise ValueError("incomplete PE optional header")
        imports_rva, imports_size = struct.unpack_from("<II", data, optional + 120)
        sections = optional + optional_size

        def offset(rva):
            for index in range(count):
                section = sections + 40 * index
                _, address, raw_size, raw_offset = struct.unpack_from("<IIII", data, section + 8)
                if address <= rva < address + raw_size:
                    result = raw_offset + rva - address
                    if result < len(data):
                        return result
            raise ValueError("invalid PE import address")

        dlls = set()
        for index in range(min(imports_size // 20, 4096)):
            entry = offset(imports_rva + index * 20)
            descriptor = struct.unpack_from("<IIIII", data, entry)
            if not any(descriptor):
                break
            start = offset(descriptor[3])
            end = data.index(b"\0", start, min(start + 256, len(data)))
            dlls.add(data[start:end].lower())
        if browser and b"libcef.dll" not in dlls:
            raise ValueError("Windows executable has no CEF integration but an engine was given")
        if not browser and b"libcef.dll" in dlls:
            raise ValueError("Windows executable needs CEF; package it with the pinned engine")
    except (struct.error, IndexError) as error:
        raise ValueError("truncated Windows executable") from error

def engine_files(engine, lock):
    pin = load_release_pin(lock)
    if pin.get("target") != TARGET:
        raise ValueError("Windows packaging requires an explicitly pinned Windows engine")
    engine = validate(engine, pin["provenance_sha256"], TARGET)
    required = [(engine / name, name) for name in DLLS]
    required += [(engine / name, name) for name in RESOURCES]
    required.append((engine / "v8_context_snapshot.bin", "v8_context_snapshot.bin"))
    required.append((engine / "vk_swiftshader_icd.json", "vk_swiftshader_icd.json"))
    locales = sorted((engine / "locales").glob("*.pak"))
    if not locales or not (engine / "locales/en-US.pak").is_file():
        raise ValueError("CEF locale resources are missing")
    required += [(file, "locales/" + file.name) for file in locales]
    for source, _ in required:
        if not source.is_file():
            raise ValueError(f"required Windows runtime file is missing: {source}")
    names = {name.casefold() for _, name in required}
    required += [(file, file.name) for file in sorted(engine.glob("*.dll"))
                 if file.name.casefold() not in names]
    required.append((engine / "superfik-engine.json", "superfik-engine.json"))
    return required

def stage(binary, engine, destination, lock, vc_runtime, root=ROOT):
    """Stage the payload; without an engine (none published yet) the app ships without the browser."""
    if (engine is None) != (lock is None):
        raise ValueError("give both the engine and its pin, or neither")
    binary = binary.resolve(strict=True)
    check_executable(binary, browser=engine is not None)
    if destination.exists():
        raise ValueError("payload destination already exists; choose a fresh output directory")
    required = engine_files(engine, lock) if engine is not None else []
    crt_files = sorted(vc_runtime.glob("*.dll"))
    engine_names = {name.casefold() for _, name in required} | {file.name.casefold() for file in crt_files}
    required += [(file, file.name) for file in sorted(binary.parent.glob("*.dll"))
                 if file.name.casefold() not in engine_names]
    required += [(binary, "superfik.exe")]
    for name in ("vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll", "msvcp140_1.dll"):
        if not (vc_runtime / name).is_file():
            raise ValueError(f"Visual C++ x64 redistributable is missing: {name}")
    required += [(file, file.name) for file in crt_files]
    required += [(root / name, name) for name in ("LICENSE", "NOTICE")]
    required.append((binary.parent / "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md"))
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=destination.parent) as temporary:
        payload = Path(temporary) / "payload"
        payload.mkdir()
        for source, name in required:
            output = payload / name
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, output)
        payload.rename(destination)
    return destination.resolve()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--engine", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--lock", type=Path)
    parser.add_argument("--vc-runtime", type=Path, required=True,
                        help="Visual Studio's x64/Microsoft.VC*.CRT redistributable directory")
    args = parser.parse_args()
    print(stage(args.binary, args.engine, args.output, args.lock, args.vc_runtime))

if __name__ == "__main__":
    main()
