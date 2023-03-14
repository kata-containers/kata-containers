from pathlib import Path
import sys


def replace_range(src: Path, dst: Path, begin_line: str, end_line: str):
    with open(src) as f:
        src_lines = f.read().splitlines(keepends=True)

    with open(dst) as f:
        dst_lines = f.read().splitlines(keepends=True)

    def gen():
        flag = False
        for l in dst_lines:
            if l.strip().startswith(begin_line):
                yield l
                yield from src_lines
                flag = True
            elif l.strip().startswith(end_line):
                yield l
                flag = False
            elif flag:
                continue
            else:
                yield l

    lines = list(gen())
    with open(dst, "w") as f:
        f.writelines(lines)


if __name__ == "__main__":
    src = Path(sys.argv[1])
    dst = Path(sys.argv[2])
    begin_line = sys.argv[3]
    end_line = sys.argv[4]
    replace_range(src, dst, begin_line, end_line)
