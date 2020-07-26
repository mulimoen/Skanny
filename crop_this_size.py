#! /usr/bin/env python3
import pathlib
import subprocess
import argparse
from multiprocessing import Pool


if __name__ == "__main__":
    parser = argparse.ArgumentParser("Batch crop standard sized images")
    parser.add_argument("indir", type=pathlib.Path)
    parser.add_argument("outdir", type=pathlib.Path)

    args = parser.parse_args()
    args.outdir.mkdir(exist_ok=True)

    def convert_file(infile: pathlib.Path):
        print(f"Splitting {infile}")
        newf = args.outdir.joinpath(infile.stem)

        subprocess.run(
            [
                "convert",
                str(infile),
                "+repage",
                "-crop",
                "2x2@",
                "+repage",
                f"{newf}_%d.png",
            ]
        )

    with Pool() as pool:
        print("Splitting images")
        for _ in pool.imap_unordered(convert_file, args.indir.iterdir(), chunksize=1):
            pass
