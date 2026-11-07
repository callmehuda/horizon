#!/usr/bin/env python3
"""Fail when a *-win7-windows build would not load on Windows 7.

The 32-bit builds target i686-win7-windows-msvc, whose baseline is Windows 7 SP1
(Rust 1.78 raised the *-pc-windows-* baseline to Windows 10; the win7 targets keep
the old one). A binary loads on a clean Windows 7 only if every DLL in its import
and delay-import tables is one Windows 7 ships: the api-ms-win-* and ext-ms-win-*
API sets (WaitOnAddress's api-ms-win-core-synch-l1-2-0 and friends) do not exist
there, and neither do Windows 8's combase, Windows 10's ucrtbase or icuuc. The
C runtime is linked in (crt-static), so none of those should appear either.

The file must also be a 32-bit PE (machine i386): that is the architecture the
Windows 7 builds are for, as tools/check_pe_arch.py checks for the others.
"""

import argparse
import struct
import sys
from pathlib import Path

# DLLs a clean Windows 7 SP1 (x86 and x64 alike) carries in System32. d3d11, d2d1 and
# dwrite are Windows 7's own; Vulkan and modern OpenGL drivers are loaded at run time
# (a driver that lacks them is an error message, not a load failure).
WINDOWS7_SYSTEM = {
    "advapi32.dll", "apphelp.dll", "bcrypt.dll", "cfgmgr32.dll", "comctl32.dll",
    "comdlg32.dll", "crypt32.dll", "cryptbase.dll", "d2d1.dll", "d3d9.dll", "d3d10.dll",
    "d3d11.dll", "dbghelp.dll", "dwmapi.dll", "dwrite.dll", "dxgi.dll", "gdi32.dll",
    "gdiplus.dll", "hid.dll", "imm32.dll", "iphlpapi.dll", "kernel32.dll",
    "kernelbase.dll", "mfplat.dll", "mmdevapi.dll", "msvcrt.dll", "ncrypt.dll",
    "ntdll.dll", "ole32.dll", "oleacc.dll", "oleaut32.dll", "opengl32.dll",
    "propsys.dll", "secur32.dll", "setupapi.dll", "shell32.dll", "shlwapi.dll",
    "user32.dll", "userenv.dll", "uiautomationcore.dll", "uxtheme.dll", "version.dll",
    "winmm.dll", "wintrust.dll", "ws2_32.dll", "wsock32.dll",
}

# What Windows 7 does not have, named for the error message: every API set (the schema
# itself exists, these names do not resolve), Windows 8+'s combase and shcore, the
# Universal CRT (linked in statically anyway), and Windows 10's ICU. Checked as
# patterns so an unexpected DLL names its reason.
WINDOWS7_MISSING = (
    ("api-ms-win-", "an API set, which Windows 7 does not resolve"),
    ("ext-ms-win-", "an API set, which Windows 7 does not resolve"),
    ("combase.dll", "Windows 8's, not Windows 7's (ole32 is)"),
    ("shcore.dll", "Windows 8.1's, not Windows 7's"),
    ("ucrtbase.dll", "the Universal CRT, not on Windows 7 (the C runtime is linked in)"),
    ("icu", "Windows 10's ICU, not Windows 7's"),
    ("bcryptprimitives.dll", "Windows 8's, not Windows 7's (bcrypt is)"),
    ("dcomp.dll", "Windows 8.1's, not Windows 7's"),
    ("xinput1_4.dll", "Windows 8's, not Windows 7's (xinput1_3 is)"),
    ("d3d12.dll", "not on Windows 7"),
    ("vcruntime", "the Visual C++ redistributable, not Windows 7's (the C runtime is linked in)"),
    ("msvcp", "the Visual C++ redistributable, not Windows 7's (the C runtime is linked in)"),
)


def windows7_allowed(name):
    name = name.lower()
    if name in WINDOWS7_SYSTEM:
        return True
    # Windows 7's own side-by-side CRT names (msvcr90.dll and friends) load from
    # WinSxS; a static-CRT build imports none of them, but they are not load errors.
    return name.startswith("msvcr") and name[5].isdigit()


def pe_machine_is_32bit(data):
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("not a PE file")
    return struct.unpack_from("<H", data, pe + 4)[0] == 0x14C


def pe_imports(data):
    """DLL names in a PE file's import and delay-import tables, each with the
    functions it takes: {dll: [name, ...]}, so a refused DLL can say what pulled it."""
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("not a PE file")
    sections_count, optional_size = struct.unpack_from("<H12xH", data, pe + 6)
    optional = pe + 24
    magic = struct.unpack_from("<H", data, optional)[0]
    pe32plus = magic == 0x20B
    directories = optional + (112 if pe32plus else 96)
    sections = [struct.unpack_from("<8xIIII", data, optional + optional_size + 40 * i)
                for i in range(sections_count)]

    def offset(rva):
        for size, address, raw_size, raw in sections:
            if address <= rva < address + max(size, raw_size):
                return rva - address + raw
        raise ValueError(f"RVA {rva:#x} outside every section")

    def name_at(rva):
        start = offset(rva)
        return data[start:data.index(b"\0", start)].decode("ascii")

    def thunk_names(rva):
        """The import-by-name table a thunk RVA points at (an import count for the
        check's own sanity, too)."""
        at = offset(rva)
        names = []
        while True:
            entry = struct.unpack_from("<I" if not pe32plus else "<Q", data, at)[0]
            if entry == 0:
                return names
            if entry & (0x80000000 if not pe32plus else 0x8000000000000000):
                names.append(f"ordinal{entry & 0xFFFF}")  # by ordinal: no name to read
            else:
                # IMAGE_IMPORT_BY_NAME: a 2-byte hint, then the name.
                names.append(name_at(entry + 2))
            at += 4 if not pe32plus else 8

    imports = {}
    # Directory 1: imports (20-byte descriptors, name at +12, thunks at +16 and +0);
    # 13: delay imports (32 bytes, name at +4, thunks at +20 (attributes stripped)).
    for index, size, name_field, thunk_field, mask in ((1, 20, 12, 16, 0), (13, 32, 4, 20, 1)):
        rva = struct.unpack_from("<I", data, directories + 8 * index)[0]
        if not rva:
            continue
        at = offset(rva)
        while any(data[at:at + size]):
            dll = name_at(struct.unpack_from("<I", data, at + name_field)[0])
            thunks = struct.unpack_from("<I", data, at + thunk_field)[0] & ~mask
            if thunks:
                imports.setdefault(dll, []).extend(thunk_names(thunks))
            else:
                imports.setdefault(dll, [])
            at += size
    return imports


def check(path):
    data = Path(path).read_bytes()
    if data[:2] != b"MZ":
        raise ValueError(f"{path} is not a Windows executable")
    problems = []
    if not pe_machine_is_32bit(data):
        problems.append("is not a 32-bit PE (machine i386), which Windows 7 builds are")
    imports = pe_imports(data)
    for name, functions in imports.items():
        lower = name.lower()
        if windows7_allowed(name):
            continue
        reason = next((why for start, why in WINDOWS7_MISSING if start in lower), None)
        where = f" ({', '.join(functions)})" if functions else ""
        if reason:
            problems.append(f"imports {name}{where}: {reason}")
        else:
            problems.append(f"imports {name}{where}, which a clean Windows 7 does not ship")
    names = sorted(name.lower() for name in imports)
    print(f"{path}: {', '.join(names)}")
    for problem in problems:
        print(f"  {path} {problem}", file=sys.stderr)
    return not problems


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("binaries", nargs="+")
    args = parser.parse_args()
    ok = all([check(path) for path in args.binaries])
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
