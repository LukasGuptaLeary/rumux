"""Symbolize GDB's native Windows addresses with the matching built PE/PDB.

Diagnostics contain function/source locations only, never locals or memory dumps.
"""
import re
import shutil
import struct
import subprocess
from pathlib import Path


def symbolize(app, trace):
    tool = shutil.which("llvm-symbolizer") or r"C:\Program Files\LLVM\bin\llvm-symbolizer.exe"
    if not Path(tool).is_file():
        return "LLVM symbolizer is unavailable."
    match = re.search(r"0x([0-9a-fA-F]+)\s*-\s*0x[0-9a-fA-F]+ is \.text\s*$", trace, re.M)
    if not match:
        return "GDB did not report the executable's loaded text section."
    with Path(app).open("rb") as stream:
        header = stream.read(64)
        assert header[:2] == b"MZ"
        pe = struct.unpack_from("<I", header, 60)[0]
        stream.seek(pe)
        header = stream.read(24)
        assert header[:4] == b"PE\0\0"
        count, optional_size = struct.unpack_from("<H", header, 6)[0], struct.unpack_from("<H", header, 20)[0]
        optional = stream.read(optional_size)
        assert struct.unpack_from("<H", optional)[0] == 0x20b, "Expected PE64"
        preferred_base = struct.unpack_from("<Q", optional, 24)[0]
        image_size = struct.unpack_from("<I", optional, 56)[0]
        text_rva = None
        for _ in range(count):
            section = stream.read(40)
            if section[:8].rstrip(b"\0") == b".text":
                text_rva = struct.unpack_from("<I", section, 12)[0]
        assert text_rva is not None
    loaded_base = int(match[1], 16) - text_rva
    addresses = list(dict.fromkeys(int(address, 16) for address in
        re.findall(r"^#\d+\s+0x([0-9a-fA-F]+)", trace, re.M)))
    addresses = [address for address in addresses if loaded_base <= address < loaded_base + image_size][:120]
    if not addresses:
        return "No executable frame addresses were captured."
    result = subprocess.run([tool, "--obj", str(app), "--adjust-vma", str(loaded_base - preferred_base),
                             "--print-address", "--pretty-print", "--no-inlines"],
        input="".join(f"0x{address:x}\n" for address in addresses),
        capture_output=True, text=True, timeout=30)
    return result.stdout + result.stderr
