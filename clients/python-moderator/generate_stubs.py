#!/usr/bin/env python3
"""
Compiles proto/game.proto and proto/user.proto into Python gRPC stubs.
Run once before first launch (and again whenever the .proto files change):

    python generate_stubs.py

grpcio-tools generates bare `import foo_pb2` statements in the *_grpc.py
files, which breaks when the stubs live inside a package.  This script
patches those lines to relative imports (`from . import foo_pb2`) after
every compilation so the package structure works correctly.
"""
import re
import subprocess
import sys
import pathlib

PROTO_DIR = (pathlib.Path(__file__).parent.parent.parent / "proto").resolve()
OUT_DIR = pathlib.Path(__file__).parent / "grpc_generated"

OUT_DIR.mkdir(exist_ok=True)
(OUT_DIR / "__init__.py").touch()

# Compile
for proto_file in ["user.proto", "game.proto"]:
    print(f"  compiling {proto_file} …")
    subprocess.run(
        [
            sys.executable, "-m", "grpc_tools.protoc",
            f"-I{PROTO_DIR}",
            f"--python_out={OUT_DIR}",
            f"--grpc_python_out={OUT_DIR}",
            str(PROTO_DIR / proto_file),
        ],
        check=True,
    )

# Patch bare imports → relative imports in every generated *_grpc.py file.
# e.g.  import game_pb2 as game__pb2
#   →   from . import game_pb2 as game__pb2
_BARE_IMPORT = re.compile(r'^import (\w+_pb2)(.*)', re.MULTILINE)

for grpc_file in OUT_DIR.glob("*_grpc.py"):
    original = grpc_file.read_text(encoding="utf-8")
    patched = _BARE_IMPORT.sub(r'from . import \1\2', original)
    if patched != original:
        grpc_file.write_text(patched, encoding="utf-8")
        print(f"  patched relative imports in {grpc_file.name}")

print(f"\nDone. Stubs written to: {OUT_DIR}")
