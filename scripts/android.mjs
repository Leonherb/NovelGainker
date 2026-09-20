import { spawn, spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  openSync,
  readSync,
  closeSync,
  readFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const toolRoot = path.join(root, "target/android-toolchain");
const appRoot = path.join(root, "apps/desktop");
const androidRoot = path.join(appRoot, "src-tauri/gen/android");
const executable = (name) => `${name}${process.platform === "win32" ? ".exe" : ""}`;
const env = { ...process.env };
for (const key of Object.keys(env)) {
  if (/(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|openai|deepseek)/iu.test(key)) {
    delete env[key];
  }
}
const localConfig = path.join(toolRoot, "environment.json");
const configured = existsSync(localConfig) ? JSON.parse(readFileSync(localConfig, "utf8")) : {};
const javaHome = env.JAVA_HOME ?? configured.javaHome ?? "";
const sdkHome = env.ANDROID_HOME ?? env.ANDROID_SDK_ROOT ?? configured.sdkHome ?? "";
const ndkHome = env.NDK_HOME ?? (sdkHome ? path.join(sdkHome, "ndk/27.2.12479018") : "");
const existingPathKey = Object.keys(env).find((key) => key.toLowerCase() === "path");
const existingPath = existingPathKey ? env[existingPathKey] : "";
if (existingPathKey) delete env[existingPathKey];
env.PATH = [
  javaHome && path.join(javaHome, "bin"),
  sdkHome && path.join(sdkHome, "platform-tools"),
  existingPath,
]
  .filter(Boolean)
  .join(path.delimiter);
if (javaHome) env.JAVA_HOME = javaHome;
if (sdkHome) env.ANDROID_HOME = sdkHome;
if (ndkHome) env.NDK_HOME = ndkHome;
env.GRADLE_USER_HOME = path.join(toolRoot, "gradle");
env.ANDROID_USER_HOME = path.join(toolRoot, "android-user");
env.CARGO_NET_OFFLINE = "true";
// Windows AF_UNIX cannot reliably use inherited 8.3 TEMP paths.
if (modeNeedsBuild(process.argv[2])) {
  env.TEMP = path.join(toolRoot, "tmp");
  env.TMP = env.TEMP;
  mkdirSync(env.TEMP, { recursive: true });
}
function modeNeedsBuild(mode) {
  return mode === "build-debug" || mode === "build-release";
}

function probe(command, args) {
  const result = spawnSync(command, args, { cwd: root, env, encoding: "utf8", windowsHide: true });
  return {
    ok: !result.error && result.status === 0,
    output: `${result.stdout ?? ""}${result.stderr ?? ""}`.trim(),
  };
}

const rust = probe("rustup", ["target", "list", "--installed"]);
const checks = {
  java: Boolean(
    javaHome &&
    existsSync(path.join(javaHome, "bin", executable("java"))) &&
    probe(path.join(javaHome, "bin", executable("java")), ["-version"]).ok,
  ),
  sdkPlatform36: Boolean(
    sdkHome && existsSync(path.join(sdkHome, "platforms/android-36/android.jar")),
  ),
  buildTools36: Boolean(
    sdkHome && existsSync(path.join(sdkHome, "build-tools/36.0.0/source.properties")),
  ),
  platformTools: Boolean(
    sdkHome && existsSync(path.join(sdkHome, "platform-tools", executable("adb"))),
  ),
  ndk27: Boolean(ndkHome && existsSync(path.join(ndkHome, "source.properties"))),
  rustAarch64: rust.ok && rust.output.split(/\r?\n/u).includes("aarch64-linux-android"),
  tauriCli: existsSync(path.join(appRoot, "node_modules/@tauri-apps/cli/tauri.js")),
};
const mode = process.argv[2] ?? "doctor";
if (!["doctor", "init", "build-debug", "build-release"].includes(mode) || process.argv.length > 3) {
  throw new Error("Usage: node scripts/android.mjs doctor|init|build-debug|build-release");
}
const missing = Object.entries(checks)
  .filter(([, ready]) => !ready)
  .map(([name]) => name);
if (mode === "doctor") {
  console.log(
    JSON.stringify(
      {
        checks,
        initialized: existsSync(path.join(androidRoot, "gradlew.bat")),
        ready: missing.length === 0,
      },
      null,
      2,
    ),
  );
  process.exit(missing.length === 0 ? 0 : 1);
}
if (missing.length) {
  throw new Error(
    `Android build dependencies missing: ${missing.join(", ")}. See docs/development-android.md.`,
  );
}
if (mode === "init" && existsSync(path.join(androidRoot, "app/build.gradle.kts"))) {
  console.log("[android] Existing native project preserved; initialization skipped.");
  process.exit(0);
}
if (mode !== "init" && !existsSync(path.join(androidRoot, "app/build.gradle.kts"))) {
  throw new Error("Android project is not initialized; run pnpm android:init first.");
}
const args =
  mode === "init"
    ? ["android", "init", "--ci", "--skip-targets-install"]
    : [
        "android",
        "build",
        "--ci",
        "--apk",
        "--target",
        "aarch64",
        ...(mode === "build-debug" ? ["--debug"] : []),
        "--",
        "--locked",
        "--offline",
      ];
async function run(command, commandArgs, cwd) {
  return await new Promise((resolve, reject) => {
    const child = spawn(command, commandArgs, { cwd, env, windowsHide: true });
    let tail = "";
    for (const [stream, output] of [
      [child.stdout, process.stdout],
      [child.stderr, process.stderr],
    ]) {
      stream.on("data", (chunk) => {
        output.write(chunk);
        tail = (tail + chunk.toString()).slice(-65536);
      });
    }
    child.on("error", reject);
    child.on("close", (status) => resolve({ status: status ?? 1, tail }));
  });
}
const result = await run(
  process.execPath,
  [path.join(appRoot, "node_modules/@tauri-apps/cli/tauri.js"), ...args],
  appRoot,
);
if (result.status === 0) process.exit(0);
// Tauri compiles first, then links its verified output into jniLibs. Only this
// precise Windows permission failure permits a copy; compilation errors fail closed.
const noSymlinkPermission =
  process.platform === "win32" &&
  modeNeedsBuild(mode) &&
  result.tail.includes("Creation symbolic link is not allowed for this system.") &&
  result.tail.includes("symlinking lib") &&
  result.tail.includes("libnovelgainker_desktop_lib.so");
if (!noSymlinkPermission) process.exit(result.status);
const profile = mode === "build-debug" ? "debug" : "release";
const source = path.join(
  root,
  "target/aarch64-linux-android",
  profile,
  "libnovelgainker_desktop_lib.so",
);
const header = Buffer.alloc(20);
const fd = openSync(source, "r");
try {
  readSync(fd, header, 0, header.length, 0);
} finally {
  closeSync(fd);
}
if (
  header.toString("hex", 0, 4) !== "7f454c46" ||
  header[4] !== 2 ||
  header[5] !== 1 ||
  header.readUInt16LE(18) !== 183
) {
  throw new Error("Refusing to package a library that is not an ARM64 ELF.");
}
const nativeDir = path.join(androidRoot, "app/src/main/jniLibs/arm64-v8a");
mkdirSync(nativeDir, { recursive: true });
copyFileSync(source, path.join(nativeDir, path.basename(source)));
console.log(
  "[android] Native compilation succeeded; packaging its ARM64 output by copy because Windows denied symlinks.",
);
const variant = profile === "debug" ? "Debug" : "Release";
// Invoke the wrapper JAR directly: no shell interpolation or global environment changes.
const packaged = await run(
  path.join(javaHome, "bin", executable("java")),
  [
    "-classpath",
    path.join(androidRoot, "gradle/wrapper/gradle-wrapper.jar"),
    "org.gradle.wrapper.GradleWrapperMain",
    "--no-daemon",
    `:app:assembleArm64${variant}`,
    "-PtargetList=aarch64",
    "-ParchList=arm64",
    "-PabiList=arm64-v8a",
    "-x",
    `:app:rustBuildArm64${variant}`,
  ],
  androidRoot,
);
process.exit(packaged.status);
