"""Fetch a pinned public engine release, verify SHA-256 and stage it safely."""

import argparse
from pathlib import Path, PurePosixPath
import posixpath
import tarfile
import tempfile
import urllib.request

from cef_artifact import PLATFORMS, ROOT, TARGET, digest, load_release_pin, platform_for, validate

def unpack(archive, destination, checksum, expected_provenance=None, target=TARGET):
    prefix = platform_for(target)[1]
    if digest(archive) != checksum:
        raise ValueError("engine archive checksum mismatch")
    if destination.exists():
        raise ValueError("engine destination already exists")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=destination.parent) as temporary:
        with tarfile.open(archive, "r:gz") as source:
            members = source.getmembers()
            links = {PurePosixPath(item.name).as_posix().casefold()
                     for item in members if item.issym()}
            regular = {item.name for item in members if item.isfile()}
            seen = set()
            if sum(item.size for item in members) > 8 * 1024**3:
                raise ValueError("engine archive exceeds unpacked size limit")
            for item in members:
                path = PurePosixPath(item.name)
                if target.endswith("windows-msvc"):
                    reserved = {"con", "prn", "aux", "nul"} | {
                        f"{base}{number}" for base in ("com", "lpt") for number in range(1, 10)
                    }
                    if (item.issym() or item.islnk() or "\\" in item.name
                            or any(":" in part or part.rstrip(" .") != part
                                   or part.split(".")[0].casefold() in reserved
                                   for part in path.parts)):
                        raise ValueError("unsafe Windows engine archive entry")
                if path.as_posix().casefold() in seen:
                    raise ValueError("duplicate engine archive path")
                seen.add(path.as_posix().casefold())
                if path.is_absolute() or '..' in path.parts or not path.parts or path.parts[0] != prefix:
                    raise ValueError("engine archive path escapes its root")
                if any(parent.as_posix().casefold() in links for parent in path.parents):
                    raise ValueError("engine archive writes through a symlink")
                if not (item.isfile() or item.isdir() or item.issym() or item.islnk()):
                    raise ValueError("unsupported engine archive entry")
                if item.issym() or item.islnk():
                    if item.islnk() and item.linkname not in regular:
                        raise ValueError("engine hardlink must target a regular archive file")
                    link_target = item.linkname if item.islnk() else posixpath.join(str(path.parent), item.linkname)
                    link_target = PurePosixPath(posixpath.normpath(link_target))
                    if link_target.is_absolute() or not link_target.parts or link_target.parts[0] != prefix:
                        raise ValueError("engine archive link escapes its root")
                    if item.islnk() and any(parent.as_posix().casefold() in links for parent in link_target.parents):
                        raise ValueError("engine archive hardlink traverses a symlink")
            source.extractall(temporary, members=members)
        engine = Path(temporary) / prefix
        validate(engine, expected_provenance, target)
        engine.rename(destination)
    return destination.resolve()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", type=Path, default=ROOT / "scripts/cef-release.lock.json")
    parser.add_argument("--archive", type=Path, help="Use a local candidate archive with the same pinned checksum")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--target", choices=PLATFORMS)
    args = parser.parse_args()
    pin = load_release_pin(args.lock)
    target = pin.get("target", TARGET)
    if args.target and args.target != target:
        raise ValueError("engine release pin does not match requested target")
    cache = ROOT / "target/browser-engine" / pin["tag"]
    if "target" in pin:
        cache = cache / target
    output = (args.output or cache).resolve()
    receipt = output.parent / (output.name + ".sha256")
    if output.exists():
        if not receipt.is_file() or receipt.read_text().strip() != pin["sha256"]:
            raise ValueError("cached engine has no matching checksum receipt")
        print(validate(output, pin["provenance_sha256"], target))
        return
    with tempfile.TemporaryDirectory() as temporary:
        archive = args.archive
        if archive is None:
            archive = Path(temporary) / "engine.tar.gz"
            url = f"https://github.com/{pin['repository']}/releases/download/{pin['tag']}/{pin['asset']}"
            with urllib.request.urlopen(url, timeout=60) as response, archive.open("wb") as file:
                total = 0
                while True:
                    chunk = response.read(1024 * 1024)
                    if not chunk:
                        break
                    total += len(chunk)
                    if total >= 2 * 1024**3:
                        raise ValueError("engine download exceeds size limit")
                    file.write(chunk)
        installed = unpack(archive, output, pin["sha256"], pin["provenance_sha256"], target)
        receipt.write_text(pin["sha256"] + "\n")
        print(installed)

if __name__ == "__main__":
    main()
