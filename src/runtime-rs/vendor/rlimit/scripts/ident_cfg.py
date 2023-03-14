import sys

from . import libc_source

if __name__ == "__main__":
    ident = sys.argv[1]
    indent = int(sys.argv[2])
    inverse = len(sys.argv) >= 4 and sys.argv[3] == "inverse"

    selectors = libc_source.calc_selectors(libc_source.search_ident(f"{ident}.+", f".+?({ident}).+"))
    selectors = sorted(selectors[ident].values())
    print(f"// generated from rust-lang/libc {libc_source.COMMIT_HASH}")
    print(libc_source.calc_cfg(selectors, indent=indent, inverse=inverse))
