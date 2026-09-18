#!/usr/bin/env python3
"""Reproduce #184 in an isolated staged renderer, optionally recording observations."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--candidate", action="store_true")
parser.add_argument("--record", action="store_true")
parser.add_argument("--clippy", action="store_true", help="check all native probe targets with warnings denied")
parser.add_argument("--browser", type=Path)
parser.add_argument("--driver", type=Path)
parser.add_argument("--fetch", action="store_true", help="populate the locked dependency cache")
parser.add_argument("--initialize-lock", action="store_true", help="seed the probe lock from production (maintenance only)")
args = parser.parse_args()
if bool(args.browser) != bool(args.driver):
    parser.error("--browser and --driver must be supplied together")
here = Path(__file__).resolve().parent
root = here.parent.parent
if args.initialize_lock:
    shutil.copy(root / "Cargo.lock", here / "Cargo.lock")
    subprocess.run(["rustup", "run", "1.97.1", "cargo", "fetch", "--offline", "--manifest-path", str(here / "Cargo.toml")], check=True)
    sys.exit(0)
mode = "candidate" if args.candidate else "current"
target = "wasm" if args.browser else "native"
expected = here / f"expected-{mode}-{target}.json"
env = dict(os.environ)
env.update(KURBO_EXPECTED=str(expected), KURBO_RECORD="1" if args.record else "0")
env["RUSTC"] = subprocess.check_output(["rustup", "which", "--toolchain", "1.97.1", "rustc"], text=True).strip()
env["RUSTDOC"] = subprocess.check_output(["rustup", "which", "--toolchain", "1.97.1", "rustdoc"], text=True).strip()
env["PATH"] = str(Path(env["RUSTC"]).parent) + os.pathsep + env["PATH"]

(root / "target").mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix="kurbo-evaluation-", dir=root / "target") as temporary:
    stage = Path(temporary)
    # A small workspace shell; large assets remain read-only path dependencies.
    shutil.copy(root / "Cargo.toml", stage / "Cargo.toml")
    (stage / "crates").mkdir()
    for crate in (root / "crates").iterdir():
        if crate.name == "excalidraw-render":
            shutil.copytree(crate, stage / "crates" / crate.name)
        else:
            (stage / "crates" / crate.name).symlink_to(crate, target_is_directory=True)
    (stage / "xtask").symlink_to(root / "xtask", target_is_directory=True)
    probe = stage / "tools" / "kurbo-evaluation"
    shutil.copytree(here, probe, ignore=shutil.ignore_patterns("target", "__pycache__"))
    if args.candidate:
        geometry = stage / "crates/excalidraw-render/src/render/geometry.rs"
        original = geometry.read_text()
        geometry.with_suffix(".rs.original").write_text(original)
        start = original.index("fn include_cubic_bounds(")
        end = original.index("pub(super) enum GeneratedGeometry", start)
        geometry.write_text(original[:start] + (here / "adapter.rs").read_text() + "\n" + original[end:])
        manifest = stage / "crates/excalidraw-render/Cargo.toml"
        text = manifest.read_text().replace("[dependencies]\n", '[dependencies]\nkurbo = { version = "=0.11.3", default-features = false, features = ["libm"] }\n')
        manifest.write_text(text)
        # The candidate adds one edge to an already locked package. Keep all
        # resolved versions/checksums fixed, rather than invoking an update.
        lock = probe / "Cargo.lock"
        text = lock.read_text()
        start = text.index('name = "excalidraw-render"\n')
        pos = text.index("dependencies = [\n", start) + len("dependencies = [\n")
        lock.write_text(text[:pos] + ' "kurbo 0.11.3",\n' + text[pos:])
    common = ["--manifest-path", str(probe / "Cargo.toml"), "--locked"]
    cargo = ["rustup", "run", "1.97.1", "cargo"]
    if args.fetch:
        subprocess.run(cargo + ["fetch"] + common, env=env, check=True)
    common += ["--offline"]
    tree = subprocess.check_output(cargo + ["tree", "-e", "features", "-i", "kurbo@0.11.3"] + common + (["--target", "wasm32-unknown-unknown"] if args.browser else []), env=env, text=True)
    feature_log = root / "target" / f"kurbo-evaluation-{mode}-{target}-features.txt"
    feature_log.write_text(tree)
    print(f"Resolved feature graph: {feature_log}", flush=True)
    common += ["--target-dir", str(root / "target" / f"kurbo-evaluation-{mode}-{target}")]
    if args.clippy:
        subprocess.run(cargo + ["clippy", "--all-targets"] + common + ["--", "-D", "warnings"], env=env, check=True)
    elif args.browser:
        config = {"goog:chromeOptions":{"binary":str(args.browser.resolve(strict=True)),"args":["--no-sandbox","--disable-dev-shm-usage"]}}
        webdriver = stage / "webdriver.json"
        webdriver.write_text(json.dumps(config))
        env.update(CHROMEDRIVER=str(args.driver.resolve(strict=True)),
                   CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="wasm-bindgen-test-runner",
                   CARGO_PROFILE_TEST_DEBUG="0", WASM_BINDGEN_TEST_ONLY_WEB="1",
                   WASM_BINDGEN_TEST_TIMEOUT="300", WASM_BINDGEN_TEST_WEBDRIVER_JSON=str(webdriver))
        command = cargo + ["test"] + common + ["--target", "wasm32-unknown-unknown", "--lib", "--", "--nocapture"]
        run = subprocess.run(command, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        log = root / "target" / f"kurbo-evaluation-{mode}-browser.log"
        log.write_text(run.stdout)
        if run.returncode:
            print(run.stdout)
            run.check_returncode()
        if args.record:
            marker = "KURBO_REPORT="
            observations = [json.loads(line.split(marker, 1)[1]) for line in run.stdout.splitlines() if marker in line]
            assert len(observations) == 1, f"expected one report; see {log}"
            expected.write_text(json.dumps(observations[0], indent=2, sort_keys=True) + "\n")
        print(f"Browser observation passed; log: {log}")
    else:
        command = cargo + (["run"] + common + ["--", "--record"] if args.record else ["test"] + common)
        subprocess.run(command, env=env, check=True)
