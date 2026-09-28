import { WASI } from "node:wasi";
import { readFileSync } from "node:fs";
const [wasmPath, ...args] = process.argv.slice(2);
const wasi = new WASI({ version: "preview1", args: ["krilla-spike", ...args], env: {}, preopens: { "/": "/" } });
const module = await WebAssembly.compile(readFileSync(wasmPath));
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());
process.exitCode = wasi.start(instance);
