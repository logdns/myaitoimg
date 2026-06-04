# myaitoimg

基于 Rust + Tauri + Vue 3 的节点式图片生成工作台。

本仓库是公开项目。代码中不包含任何可用的 OpenAI、Gemini、中转 API Key 或 Tauri 发布签名私钥；桌面端自动更新依赖公开 GitHub Release 中的 `latest.json` 和安装包资产。

## 功能

- 节点画布式界面：提示词、图片处理、结果预览。
- 可滚动画布和可拖拽节点：小窗口下可通过滚动查看完整节点，图片处理节点内容过长时在节点内滚动。
- OpenAI 图片接口：默认模型 `gpt-image-2`，支持文生图、图片编辑、参考图、蒙版、尺寸、质量、格式、透明背景、压缩参数。
- Gemini 图片接口：`gemini-3-pro-image-preview`，支持文生图、图生图/局部编辑、多参考图、比例、1K/2K/4K、搜索接地和思考模式。
- 中转 API：默认按 OpenAI Images 兼容格式请求，支持自定义模型、Endpoint、Edit Endpoint、鉴权 Header 和鉴权前缀。
- Tauri bundle 配置为 `targets: "all"`，可在 macOS、Windows、Linux 上分别构建安装包。

> 说明：`gpt-image-2` 官方模型页显示它支持文本/图片输入和图片输出，并支持 `v1/images/generations`、`v1/images/edits`。

参考资料：

- OpenAI Image Generation: https://platform.openai.com/docs/guides/image-generation
- OpenAI Images API Reference: https://platform.openai.com/docs/api-reference/images
- OpenAI GPT Image 2 Model: https://developers.openai.com/api/docs/models/gpt-image-2
- Google Gemini API Image Generation: https://ai.google.dev/gemini-api/docs/image-generation

## 开发

```bash
npm install
npm run dev
```

## 桌面开发

需要先安装 Rust 工具链和对应平台依赖：

```bash
rustup default stable
npm run tauri:dev
```

## 安全和隐私

- API Key 由用户在本机客户端设置中自行填写，仓库只提供空值和占位符。
- 客户端会把接口配置保存到浏览器/Tauri WebView 的 `localStorage`，便于下次打开继续使用；这不是加密密钥库，不建议在多人共用系统账户中保存高权限 Key。
- 桌面端生成请求由 Tauri 后端发起，前端不会把 API Key 发送到本项目服务器。本项目没有自建后端。
- 发布签名私钥通过 GitHub Actions Secrets 注入：`TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`，不要写入源码、README、日志或 Release 资产。
- `src-tauri/tauri.conf.json` 中的 `pubkey` 是 Tauri updater 公钥，可以公开，用于校验已签名更新包。
- 自动更新的 `latest.json` 和安装包需要匿名可访问；如果仓库或 Release 资产不可公开访问，客户端检查更新会失败。

## 打包

```bash
npm run tauri:build
```

发布前建议先检查版本一致性，避免安装器或更新元数据显示旧版本：

```bash
npm run check:versions
```

Tauri 不能在单台机器上直接生成所有平台的原生安装包。本项目已提供 `.github/workflows/build.yml`，GitHub Actions 会分别在 macOS、Windows、Linux runner 上构建客户端。

### GitHub Actions 构建三端客户端

触发方式：

- 推送到 `main`：自动构建 macOS、Windows、Linux 三个平台，并发布当前版本的 Release 安装包。
- 在 GitHub Actions 页面手动运行 `Build Desktop Clients`：也会构建并发布当前版本的 Release 安装包。

构建完成后，在对应 workflow run 的 `Artifacts` 中下载：

- `myaitoimg-macos`：macOS `.dmg` / `.app.tar.gz`
- `myaitoimg-windows`：Windows `.msi` / `.exe`
- `myaitoimg-linux`：Linux `.AppImage` / `.deb` / `.rpm`

发布新版本时，需要先同步更新以下文件中的版本号，再推送到 `main`：

- `package.json`
- `package-lock.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/INSTALLER.rtf`
- `src-tauri/INSTALLER.zh-CN.txt`

CI 会检查 `package.json`、`package-lock.json`、`src-tauri/tauri.conf.json`、`Cargo.toml`、`Cargo.lock`、安装协议文本中的版本号是否一致。Windows release 构建使用 GUI 子系统，不应弹出额外控制台窗口。

## 接口说明

OpenAI 和中转 API 按 OpenAI Images 兼容格式请求：

```json
{
  "model": "gpt-image-2",
  "prompt": "...",
  "size": "1024x1024",
  "quality": "high",
  "n": 1,
  "response_format": "b64_json",
  "output_format": "png"
}
```

编辑/局部/变体模式会优先使用 `Edit Endpoint`，并携带参考图和可选蒙版。

Gemini 接口按 `models/{model}:generateContent` 请求，并从响应的 `inlineData` 中读取图片。参考图会作为 `inlineData` 一起发送。
