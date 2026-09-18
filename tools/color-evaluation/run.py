#!/usr/bin/env python3
"""Run the isolated #185 observations (production source is read-only)."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--record", action="store_true")
parser.add_argument("--browser", type=Path)
parser.add_argument("--driver", type=Path)
parser.add_argument("--fetch", action="store_true")
parser.add_argument("--initialize-lock", action="store_true")
parser.add_argument("--clippy", action="store_true")
args = parser.parse_args()
if bool(args.browser) != bool(args.driver):
    parser.error("--browser and --driver are required together")
here = Path(__file__).resolve().parent
root = here.parent.parent
cargo = ["rustup", "run", "1.97.1", "cargo"]
env = dict(os.environ)
for name in ["rustc", "rustdoc"]:
    env[name.upper()] = subprocess.check_output(["rustup", "which", "--toolchain", "1.97.1", name], text=True).strip()
env["PATH"] = str(Path(env["RUSTC"]).parent) + os.pathsep + env["PATH"]
env["COLOR_RECORD"] = "1" if args.record else "0"
common = ["--manifest-path", str(here / "Cargo.toml")]
if args.initialize_lock:
    shutil.copy(root / "Cargo.lock", here / "Cargo.lock")
    subprocess.run(cargo + ["fetch"] + common, env=env, check=True)
    raise SystemExit(0)
common += ["--locked"]
if args.fetch:
    subprocess.run(cargo + ["fetch"] + common, env=env, check=True)
common += ["--offline"]
target = "wasm32-unknown-unknown" if args.browser else "x86_64-unknown-linux-gnu"
out = root / "target" / "color-evaluation"
out.mkdir(parents=True, exist_ok=True)
for package in ["palette", "color", "libm", "num-traits", "csscolorparser"]:
    tree = subprocess.check_output(cargo + ["tree", "-e", "features", "-i", package, "--target", target] + common, env=env, text=True)
    (out / f"{target}-{package}-features.txt").write_text(tree)
common += ["--target-dir", str(out)]
if args.clippy:
    subprocess.run(cargo + ["clippy", "--all-targets"] + common + ["--", "-D", "warnings"], env=env, check=True)
elif args.browser:
    config = out / "webdriver.json"
    config.write_text(json.dumps({"goog:chromeOptions": {"binary": str(args.browser.resolve(strict=True)), "args": ["--no-sandbox", "--disable-dev-shm-usage"]}}))
    env.update(CHROMEDRIVER=str(args.driver.resolve(strict=True)),
               CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="wasm-bindgen-test-runner",
               CARGO_PROFILE_TEST_DEBUG="0", WASM_BINDGEN_TEST_ONLY_WEB="1",
               WASM_BINDGEN_TEST_TIMEOUT="300", WASM_BINDGEN_TEST_WEBDRIVER_JSON=str(config))
    run = subprocess.run(cargo + ["test"] + common + ["--target", target, "--lib", "--", "--nocapture"], env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    (out / "browser.log").write_text(run.stdout)
    if run.returncode:
        print(run.stdout)
        run.check_returncode()
    if args.record:
        rows = [json.loads(line.split("COLOR_REPORT=", 1)[1]) for line in run.stdout.splitlines() if "COLOR_REPORT=" in line]
        assert len(rows) == 1
        (here / "expected-wasm.json").write_text(json.dumps(rows[0], indent=2, sort_keys=True) + "\n")
    print(f"Browser observation passed; log: {out / 'browser.log'}")
else:
    subprocess.run(cargo + (["run"] + common + ["--", "--record"] if args.record else ["test"] + common), env=env, check=True)
