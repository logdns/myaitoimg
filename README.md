# myaitoimg

基于 Rust + Tauri + Vue 3 的节点式图片生成工作台。

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

- 推送到 `main`：自动构建 macOS、Windows、Linux 三个平台。
- 创建 `v*` 标签，例如 `v0.1.0`：自动构建并发布 Release 安装包。
- 在 GitHub Actions 页面手动运行 `Build Desktop Clients`：可选择是否发布 Release。

构建完成后，在对应 workflow run 的 `Artifacts` 中下载：

- `myaitoimg-macos`：macOS `.dmg` / `.app.tar.gz`
- `myaitoimg-windows`：Windows `.msi` / `.exe`
- `myaitoimg-linux`：Linux `.AppImage` / `.deb` / `.rpm`

发布版本示例：

```bash
git tag v0.1.0
git push origin v0.1.0
```

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
