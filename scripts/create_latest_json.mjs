import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, extname, join } from "node:path";

const [artifactRoot, version, repository] = process.argv.slice(2);

if (!artifactRoot || !version || !repository) {
  console.error("Usage: node scripts/create_latest_json.mjs <artifact-root> <version> <owner/repo>");
  process.exit(1);
}

function walk(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    return entry.isDirectory() ? walk(path) : [path];
  });
}

function releaseUrl(fileName) {
  return `https://github.com/${repository}/releases/download/v${version}/${encodeURIComponent(fileName)}`;
}

function signatureFor(path) {
  const signaturePath = `${path}.sig`;
  if (!existsSync(signaturePath)) {
    throw new Error(`Missing signature file: ${signaturePath}`);
  }
  return readFileSync(signaturePath, "utf8").trim();
}

const files = walk(artifactRoot);
const installers = files.filter((file) => {
  const name = basename(file);
  if (name.endsWith(".sig")) return false;
  return [".dmg", ".msi", ".exe", ".AppImage"].includes(extname(name));
});

const macos = files.find((file) => basename(file).endsWith(".app.tar.gz"));
const windows = installers.find((file) => basename(file).endsWith("_x64-setup.exe"))
  ?? installers.find((file) => basename(file).endsWith("_x64_zh-CN.msi"));
const linux = installers.find((file) => basename(file).endsWith("_amd64.AppImage"));

const platforms = {};
if (macos) {
  platforms["darwin-aarch64"] = {
    signature: signatureFor(macos),
    url: releaseUrl(basename(macos)),
  };
}
if (windows) {
  platforms["windows-x86_64"] = {
    signature: signatureFor(windows),
    url: releaseUrl(basename(windows)),
  };
}
if (linux) {
  platforms["linux-x86_64"] = {
    signature: signatureFor(linux),
    url: releaseUrl(basename(linux)),
  };
}

for (const target of ["darwin-aarch64", "windows-x86_64", "linux-x86_64"]) {
  if (!platforms[target]) {
    throw new Error(`Missing updater installer for ${target}`);
  }
}

writeFileSync(
  join(artifactRoot, "latest.json"),
  `${JSON.stringify(
    {
      version,
      notes: "修复和功能更新。客户端可自动下载并安装此版本。",
      pub_date: new Date().toISOString(),
      platforms,
    },
    null,
    2,
  )}\n`,
);
