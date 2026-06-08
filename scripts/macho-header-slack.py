#!/usr/bin/env python3
"""Report (and optionally assert) the Mach-O header slack of a thin arm64 binary.

Header slack = the zero padding between the end of the load commands and the
first section's file content. The dedicated signing worker consumes this slack
to inject the `__MCPORB,__assets` segment (+152 B load command) WITHOUT shifting
__TEXT or rebasing symbols. Built with `-Wl,-headerpad,0x1000` it is ~4096 B;
a build that dropped the flag leaves only ~56 B and injection silently falls
back to the unsignable footer path.

Slack is computed EXACTLY as the injector does — keep in sync with
MCPOrbEtc/licensing-site/orb-signer/src/inject.rs:234-251:
    slack = min(non-zero section file offset) - (mach_header_64 + sizeofcmds)

Usage:
    macho-header-slack.py <binary> [<binary> ...]          # print slack, exit 0
    macho-header-slack.py --min 1024 <binary> [<binary>]   # also assert >= min
Exit codes: 0 ok; 1 slack below --min; 2 parse error / not a thin arm64 Mach-O.
"""
import struct
import sys

MH_MAGIC_64 = 0xFEEDFACF
LC_SEGMENT_64 = 0x19
HEADER_64 = 32  # sizeof(mach_header_64)
NEW_LC_SIZE = 152  # hard floor: the +152 B segment load command the injector adds


def header_slack(path: str) -> int:
    with open(path, "rb") as fh:
        d = fh.read()
    if len(d) < HEADER_64:
        raise ValueError(f"{path}: too small to be a Mach-O")
    magic = struct.unpack("<I", d[0:4])[0]
    if magic != MH_MAGIC_64:
        raise ValueError(
            f"{path}: not a thin 64-bit little-endian Mach-O (magic={magic:#x}); "
            "FAT or non-arm64 inputs are unsupported by the injector"
        )
    ncmds = struct.unpack("<I", d[16:20])[0]
    sizeofcmds = struct.unpack("<I", d[20:24])[0]
    lc_end = HEADER_64 + sizeofcmds
    off = HEADER_64
    min_sect = None
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack("<II", d[off:off + 8])
        if cmd == LC_SEGMENT_64:
            nsects = struct.unpack("<I", d[off + 64:off + 68])[0]
            sec = off + 72
            for _s in range(nsects):
                s_size = struct.unpack("<Q", d[sec + 40:sec + 48])[0]
                s_off = struct.unpack("<I", d[sec + 48:sec + 52])[0]
                if s_off and s_size:
                    min_sect = s_off if min_sect is None else min(min_sect, s_off)
                sec += 80
        off += cmdsize
    if min_sect is None:
        raise ValueError(f"{path}: no sections with file content")
    return min_sect - lc_end


def main(argv: list[str]) -> int:
    args = argv[1:]
    minimum = None
    if args and args[0] == "--min":
        if len(args) < 2:
            print("--min requires a value", file=sys.stderr)
            return 2
        minimum = int(args[1])
        args = args[2:]
    if not args:
        print(__doc__, file=sys.stderr)
        return 2
    rc = 0
    for path in args:
        try:
            slack = header_slack(path)
        except (OSError, ValueError, struct.error) as e:
            print(e, file=sys.stderr)
            return 2
        print(f"{path}: header slack = {slack} bytes (injector needs >= {NEW_LC_SIZE})")
        if minimum is not None and slack < minimum:
            print(
                f"{path}: header slack {slack} < {minimum} — headerpad did NOT take "
                "effect; segment injection on the signing worker would fall back to "
                "footer (unsignable). Build with -Wl,-headerpad,0x1000.",
                file=sys.stderr,
            )
            rc = 1
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv))
