import { readFileSync } from "node:fs";

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function expectContains(path, expected) {
  const content = readFileSync(path, "utf8");
  if (!content.includes(expected)) {
    throw new Error(`${path} does not contain ${expected}`);
  }
}

const packageVersion = readJson("package.json").version;
const packageLockVersion = readJson("package-lock.json").version;
const tauriVersion = readJson("src-tauri/tauri.conf.json").version;
const cargoToml = readFileSync("src-tauri/Cargo.toml", "utf8");
const cargoLock = readFileSync("src-tauri/Cargo.lock", "utf8");

const cargoTomlVersion = cargoToml.match(/^version = "([^"]+)"/m)?.[1];
const cargoLockVersion = cargoLock.match(/\[\[package\]\]\r?\nname = "myaitoimg"\r?\nversion = "([^"]+)"/)?.[1];

const checks = [
  ["package-lock.json", packageLockVersion],
  ["src-tauri/tauri.conf.json", tauriVersion],
  ["src-tauri/Cargo.toml", cargoTomlVersion],
  ["src-tauri/Cargo.lock", cargoLockVersion],
];

for (const [path, version] of checks) {
  if (version !== packageVersion) {
    throw new Error(`${path} version is ${version}, expected ${packageVersion}`);
  }
}

expectContains("src-tauri/INSTALLER.rtf", `myaitoimg v${packageVersion}`);
expectContains("src-tauri/INSTALLER.zh-CN.txt", `myaitoimg v${packageVersion}`);

console.log(`All release versions match ${packageVersion}.`);
