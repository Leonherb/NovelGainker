# Android 开发与验证

当前阶段：已产出构建验证 APK，平台可行性阶段尚未通过。产品目标与用户已确认取舍见 [安卓方案](reviews/android-mobile-technical-plan-2026-09-17.zh-CN.md)。支持目标为一加 12 与 Redmi K70 至尊版，Rust ARM64 交叉编译及 APK 打包已通过，真机验证尚未进行，不能将开发 APK 视为完整手机版。

## 工具链

Windows 上复用仓库 Node/pnpm/Rust，新增项目隔离的 Temurin JDK 17、Android Command-line Tools 19.0（固定构建 13114758）、Android API 36、Build Tools 35.0.0（AGP 默认）/36.0.0、NDK 27.2.12479018 及 aarch64-linux-android Rust 目标。工具下载和 Gradle 缓存位于 gitignored 的 target/android-toolchain；脚本不修改系统级环境变量。Rust target/cache 按 rustup/Cargo 本身的用户目录管理。

先阅读并确认 [Google SDK 许可](https://developer.android.com/studio/terms)，才运行：

```powershell
powershell -NoProfile -File scripts/setup-android.ps1 -AcceptAndroidSdkLicense
pnpm android:doctor
pnpm android:init
pnpm android:build:debug
```

安装脚本校验固定下载包的官方校验值；失败保留文件供检查，不跳过校验。Cargo fetch 是首次依赖获取步骤；构建和验证保持 --locked --offline。Gradle 首次构建仍需下载 Android 构建插件和 Maven 依赖，不能称为全工具链离线构建。

android:doctor 只读检查环境，有缺项时返回非零。android:init 不覆盖已经存在的原生工程。android:build:debug 生成 arm64 开发 APK；android:build 运行 release 构建入口，正式签名配置尚待交付阶段实现，不得把未签名产物用于正式升级。

Windows 构建脚本为子进程使用项目内 TEMP/TMP，避免继承 8.3 短路径导致 Java 本地套接字失败。安装脚本首次可将系统代理的无凭据主机/端口写入项目 Gradle 缓存；已有 gradle.properties 保留，系统代理变化后需手动更新该本机文件，不提交到仓库。

若 Tauri 已完成原生编译、仅因 Windows 符号链接权限而失败，构建脚本会核验输出是 ARM64 ELF，复制到 jniLibs，再执行对应 Gradle 打包任务；其他编译错误直接失败，不使用旧包冒充成功。不修改 Windows 开发者模式或安全策略。

## 平台边界

- 安卓覆盖配置使用 com.novelgainker.mobile，开发构建追加 .debug；Windows 配置保持原标识。
- 最低 Android 14/API 34。实际两台设备的 Android/厂商系统/WebView 版本需单独记录。
- gen/android 原生工程已纳入版本管理；其局部 .gitignore 排除 Gradle 缓存、机器路径、构建产物、密钥与 Tauri 每次生成的文件。已有检出不需要重新生成原生工程，android:init 会保留它。
- Android 新建工程使用应用私有目录；已有工程只能经最近列表重新打开，外部工程导入尚未接入。安卓凭据、前台服务与持久草稿尚未实现，不宣称这些能力已可用。
- 真实 Provider 仍由用户自行测试；自动化只用隔离合成工程和受控本地 Provider。

## 验证

平台配置与构建准备改动运行 pnpm format:check、pnpm lint、pnpm typecheck，并按 CLAUDE.md 运行完整 pnpm quality 后才能声明实现阶段技术完成。安卓原生构建与双设备验收另行记录，桌面质量通过不能代替手机证据。

## 2026-09-17 构建记录

- 产物：target/android-build/NovelGainker-0.1.0-android-arm64-preview.apk，46,260,321 字节；Debug 签名，非正式发行。
- SHA-256：adc86ea82806d726f0fb6993602a2546b826cec2111f83e19d4d267292326c33
- Android v2 签名验证、ZIP CRC、zipalign -c -P 16、APK 内全部 ELF LOAD 段 16 KB 对齐均通过；仅包含 arm64-v8a 原生库。
- 原生链接按 [Android 16 KB 页面说明](https://developer.android.com/guide/practices/page-sizes)配置 NDK r27；打包校验不替代 16 KB 设备运行测试。
- pnpm android:build:debug 已通过，含 Windows 符号链接权限不足时的复制打包路径。Kotlin 跨 C:/D: 盘源码使用非增量编译以避免路径相对化异常。最终 APK 剥离原生调试符号，完整符号保留在 Cargo target 下。
- 本轮 pnpm quality 在 2026-09-17 13:35–13:57（Asia/Shanghai）通过；其证据为 target/qa/nr-603/regression-quality.json。质量检查期间继续进行了平台适配，后续最终构建改动另通过 pnpm lint:rust（原始严格参数）、format:check、lint、Rust fmt 及 Android 原生构建；不把该早期质量结果说成最后全部改动的完整重跑。
- 用户暂时无法连接手机，要求先完成构建。两款机型均尚未安装/启动/后台实测；A 阶段、B–E 阶段及任何 NR Gate 均不因本次 APK 而标完成。
- 除上述未完成功能外，reqwest 所用 Android 平台 TLS verifier 的原生初始化/Java 依赖仍需接入；不能把原生库能链接等同于 HTTPS 可用。
- 安装说明、签名证据、zipalign 输出与机器可读校验记录保存在 target/android-build/。不自动使用真实密钥、真实稿件、Provider 网络或分发安装包。
