#!/usr/bin/env python3
"""Run the exact report assertion in a browser selected explicitly by the caller."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--browser", required=True, type=Path)
parser.add_argument("--driver", required=True, type=Path)
args = parser.parse_args()
here = Path(__file__).resolve().parent
root = here.parent.parent
config = json.loads((here / "webdriver.json").read_text())
config["goog:chromeOptions"]["binary"] = str(args.browser.resolve(strict=True))
with tempfile.TemporaryDirectory(prefix="bmp-evaluation-") as temporary:
    webdriver = Path(temporary) / "webdriver.json"
    webdriver.write_text(json.dumps(config))
    env = dict(os.environ)
    env.update(
        CHROMEDRIVER=str(args.driver.resolve(strict=True)),
        CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="wasm-bindgen-test-runner",
        CARGO_PROFILE_TEST_DEBUG="0",
        WASM_BINDGEN_TEST_ONLY_WEB="1",
        WASM_BINDGEN_TEST_TIMEOUT="180",
        WASM_BINDGEN_TEST_WEBDRIVER_JSON=str(webdriver),
    )
    subprocess.run(
        ["cargo", "test", "--manifest-path", str(here / "Cargo.toml"),
         "--locked", "--offline", "--target-dir", str(root / "target/bmp-evaluation-wasm"),
         "--target", "wasm32-unknown-unknown", "--lib"],
        cwd=root, env=env, check=True,
    )
