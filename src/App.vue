<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

type ProviderKind = "openai" | "gemini" | "custom";
type ProcessMode = "generate" | "edit" | "variation";

interface ProviderConfig {
  id: string;
  name: string;
  kind: ProviderKind;
  endpoint: string;
  editEndpoint: string;
  apiKey: string;
  model: string;
  authHeader: string;
  authPrefix: string;
}

interface GeneratePayload {
  provider: ProviderConfig;
  prompt: string;
  mode: ProcessMode;
  size: string;
  aspectRatio: string;
  imageSize: string;
  quality: string;
  format: string;
  background: string;
  moderation: string;
  compression: number;
  count: number;
  temperature: number;
  searchGrounding: boolean;
  thinking: boolean;
  project: string;
  tags: string;
  referenceImages: ReferenceImage[];
  maskImage?: ReferenceImage;
}

interface GeneratedImage {
  dataUrl: string;
  mimeType: string;
  model: string;
  provider: string;
}

interface ReferenceImage {
  name: string;
  mimeType: string;
  data: string;
}

interface ImageCommandPayload {
  dataUrl: string;
  mimeType: string;
  fileName: string;
}

interface TauriWindow extends Window {
  __TAURI_INTERNALS__?: unknown;
}

type NodeId = "prompt" | "action" | "result";

interface NodePosition {
  x: number;
  y: number;
}

const storageKey = "gptimage-workbench-state";
const snapshotKey = "gptimage-workbench-snapshot";
const stateVersion = 2;

const defaultProviders: ProviderConfig[] = [
  {
    id: "openai",
    name: "OpenAI",
    kind: "openai",
    endpoint: "https://api.openai.com/v1/images/generations",
    editEndpoint: "https://api.openai.com/v1/images/edits",
    apiKey: "",
    model: "gpt-image-2",
    authHeader: "Authorization",
    authPrefix: "Bearer ",
  },
  {
    id: "gemini",
    name: "Google Gemini",
    kind: "gemini",
    endpoint: "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent",
    editEndpoint: "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent",
    apiKey: "",
    model: "gemini-3-pro-image-preview",
    authHeader: "x-goog-api-key",
    authPrefix: "",
  },
  {
    id: "custom",
    name: "中转 API",
    kind: "custom",
    endpoint: "https://你的中转站/v1/images/generations",
    editEndpoint: "https://你的中转站/v1/images/edits",
    apiKey: "",
    model: "gpt-image-2",
    authHeader: "Authorization",
    authPrefix: "Bearer ",
  },
];

const officialEndpointDefaults: Record<Exclude<ProviderKind, "custom">, Pick<ProviderConfig, "endpoint" | "editEndpoint" | "authHeader" | "authPrefix">> = {
  openai: {
    endpoint: "https://api.openai.com/v1/images/generations",
    editEndpoint: "https://api.openai.com/v1/images/edits",
    authHeader: "Authorization",
    authPrefix: "Bearer ",
  },
  gemini: {
    endpoint: "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent",
    editEndpoint: "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent",
    authHeader: "x-goog-api-key",
    authPrefix: "",
  },
};

const restored = loadState();
const providers = reactive<ProviderConfig[]>(normalizeProviders(restored?.providers));
const selectedProviderId = ref(restored?.selectedProviderId ?? "openai");
const workflowCreated = ref(restored?.stateVersion === stateVersion ? (restored?.workflowCreated ?? false) : false);
const prompt = ref(workflowCreated.value ? (restored?.prompt ?? "") : "");
const mode = ref<ProcessMode>(restored?.mode ?? "generate");
const size = ref(restored?.size ?? "1024x1024");
const aspectRatio = ref(restored?.aspectRatio ?? "1:1");
const imageSize = ref(restored?.imageSize ?? "1K");
const quality = ref(restored?.quality ?? "high");
const format = ref(restored?.format ?? "png");
const background = ref(restored?.background ?? "auto");
const moderation = ref(restored?.moderation ?? "auto");
const compression = ref(restored?.compression ?? 100);
const count = ref(restored?.count ?? 1);
const temperature = ref(restored?.temperature ?? 1);
const searchGrounding = ref(restored?.searchGrounding ?? false);
const thinking = ref(restored?.thinking ?? true);
const project = ref(restored?.project ?? "");
const tags = ref(restored?.tags ?? "");
const referenceImages = ref<ReferenceImage[]>(restored?.referenceImages ?? []);
const maskImage = ref<ReferenceImage | undefined>(restored?.maskImage);
const zoom = ref(restored?.zoom ?? 1);
const images = ref<GeneratedImage[]>(restored?.images ?? []);
const activeImageIndex = ref(restored?.activeImageIndex ?? 0);
const isRunning = ref(false);
const status = ref("文生图工作流模板已创建。");
const progressLabel = ref("等待运行");
const elapsedMs = ref(0);
const runStartedAt = ref(0);
let progressTimer: number | undefined;
const errorMessage = ref("");
const showSettings = ref(false);
const showLocalMenu = ref(false);
const nodePositions = reactive<Record<NodeId, NodePosition>>(
  restored?.nodePositions ?? {
    prompt: { x: 82, y: 330 },
    action: { x: 770, y: 118 },
    result: { x: 1328, y: 250 },
  }
);
const dragging = ref<{
  id: NodeId;
  startX: number;
  startY: number;
  originX: number;
  originY: number;
} | null>(null);

const selectedProvider = computed(() => {
  return providers.find((provider) => provider.id === selectedProviderId.value) ?? providers[0];
});

const activeImage = computed(() => images.value[activeImageIndex.value]);
const promptCount = computed(() => prompt.value.trim().length);
const providerCapability = computed(() => {
  if (selectedProvider.value.kind === "gemini") {
    return "Gemini 3 Pro Image: 文生图、图生图/局部编辑、多参考图、1K/2K/4K、搜索接地、思考模式。";
  }
  if (selectedProvider.value.kind === "custom") {
    return "中转 API: 默认按 OpenAI Images 兼容格式请求，支持自定义模型、Endpoint、鉴权头。";
  }
  return "OpenAI GPT Image: 文生图、图片编辑、多图参考、透明背景、质量/尺寸/格式控制。";
});
const canRun = computed(() => {
  const provider = selectedProvider.value;
  return Boolean(workflowCreated.value && provider.endpoint.trim() && provider.model.trim() && prompt.value.trim());
});
const isTauriRuntime = computed(() => Boolean((window as TauriWindow).__TAURI_INTERNALS__));
const elapsedText = computed(() => {
  if (!elapsedMs.value) return "0.0s";
  return `${(elapsedMs.value / 1000).toFixed(1)}s`;
});

function loadState() {
  try {
    const value = localStorage.getItem(storageKey);
    return value ? JSON.parse(value) : null;
  } catch {
    return null;
  }
}

function saveState() {
  normalizeSelectedProvider();
  localStorage.setItem(
    storageKey,
    JSON.stringify({
      providers,
      stateVersion,
      selectedProviderId: selectedProviderId.value,
      workflowCreated: workflowCreated.value,
      prompt: prompt.value,
      mode: mode.value,
      size: size.value,
      aspectRatio: aspectRatio.value,
      imageSize: imageSize.value,
      quality: quality.value,
      format: format.value,
      background: background.value,
      moderation: moderation.value,
      compression: compression.value,
      count: count.value,
      temperature: temperature.value,
      searchGrounding: searchGrounding.value,
      thinking: thinking.value,
      project: project.value,
      tags: tags.value,
      referenceImages: referenceImages.value,
      maskImage: maskImage.value,
      zoom: zoom.value,
      images: images.value,
      activeImageIndex: activeImageIndex.value,
      nodePositions,
    })
  );
}

function normalizeSelectedProvider() {
  const provider = selectedProvider.value;
  provider.endpoint = normalizeApiUrl(provider.endpoint);
  provider.editEndpoint = normalizeApiUrl(provider.editEndpoint);
  if (provider.authPrefix.trim().toLowerCase() === "bearer") {
    provider.authPrefix = "Bearer ";
  }
}

function normalizeApiUrl(value: string): string {
  return value.trim().replace(/\.comv1\//, ".com/v1/").replace(/\.netv1\//, ".net/v1/").replace(/\.orgv1\//, ".org/v1/");
}

function applyOfficialEndpointDefaults() {
  const provider = selectedProvider.value;
  if (provider.kind === "custom") {
    saveState();
    return;
  }

  const defaults = officialEndpointDefaults[provider.kind];
  provider.endpoint = defaults.endpoint;
  provider.editEndpoint = defaults.editEndpoint;
  provider.authHeader = defaults.authHeader;
  provider.authPrefix = defaults.authPrefix;
  status.value = provider.kind === "openai" ? "已使用 OpenAI 官方默认接口地址。" : "已使用 Gemini 官方默认接口地址。";
  saveState();
}

function onProviderKindChange() {
  applyOfficialEndpointDefaults();
}

async function runProcess() {
  if (!canRun.value || isRunning.value) return;

  const payloadReferenceImages = [...referenceImages.value];
  if (mode.value !== "generate" && !payloadReferenceImages.length && activeImage.value) {
    payloadReferenceImages.push(imageToReference(activeImage.value));
    referenceImages.value = payloadReferenceImages;
  }

  isRunning.value = true;
  errorMessage.value = "";
  images.value = [];
  activeImageIndex.value = 0;
  status.value = "准备请求...";
  progressLabel.value = "准备请求";
  elapsedMs.value = 0;
  runStartedAt.value = Date.now();
  progressTimer = window.setInterval(() => {
    elapsedMs.value = Date.now() - runStartedAt.value;
  }, 100);
  saveState();

  const payload: GeneratePayload = {
      provider: { ...selectedProvider.value },
      prompt: prompt.value,
      mode: mode.value,
      size: size.value,
      aspectRatio: aspectRatio.value,
      imageSize: imageSize.value,
      quality: quality.value,
      format: format.value,
      background: background.value,
      moderation: moderation.value,
      compression: compression.value,
      count: count.value,
      temperature: temperature.value,
      searchGrounding: searchGrounding.value,
      thinking: thinking.value,
      project: project.value,
      tags: tags.value,
      referenceImages: payloadReferenceImages,
      maskImage: maskImage.value,
  };

  try {
    progressLabel.value = isTauriRuntime.value ? "桌面客户端请求中" : "浏览器直连请求中";
    status.value = "请求接口中...";
    const result = isTauriRuntime.value
      ? await invoke<GeneratedImage[]>("generate_images", { payload })
      : await generateImagesInBrowser(payload);
    progressLabel.value = "解析图片";
    images.value = result.map((image) => ({ ...image, dataUrl: addCacheBust(image.dataUrl) }));
    activeImageIndex.value = 0;
    elapsedMs.value = Date.now() - runStartedAt.value;
    progressLabel.value = "完成";
    status.value = `成功 ${result.length} 张，用时 ${elapsedText.value}`;
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
    progressLabel.value = "失败";
    status.value = "失败";
  } finally {
    if (progressTimer) {
      window.clearInterval(progressTimer);
      progressTimer = undefined;
    }
    elapsedMs.value = runStartedAt.value ? Date.now() - runStartedAt.value : elapsedMs.value;
    isRunning.value = false;
    saveState();
  }
}

function addCacheBust(dataUrl: string): string {
  if (dataUrl.startsWith("data:")) return dataUrl;
  const separator = dataUrl.includes("?") ? "&" : "?";
  return `${dataUrl}${separator}t=${Date.now()}`;
}

function imageToReference(image: GeneratedImage): ReferenceImage {
  const [meta, data] = image.dataUrl.split(",");
  return {
    name: `reference-${Date.now()}.${extensionFromMime(image.mimeType)}`,
    mimeType: meta.match(/data:(.*);base64/)?.[1] ?? image.mimeType,
    data: data ?? image.dataUrl,
  };
}

function nodeStyle(id: NodeId) {
  const position = nodePositions[id];
  return {
    left: `${position.x}px`,
    top: `${position.y}px`,
  };
}

function connectionStyle(from: NodeId, to: NodeId, fromWidth: number, toHeight: number) {
  const startX = nodePositions[from].x + fromWidth;
  const startY = nodePositions[from].y + toHeight / 2;
  const endX = nodePositions[to].x;
  const endY = nodePositions[to].y + toHeight / 2;
  const left = Math.min(startX, endX);
  const top = Math.min(startY, endY) - 40;
  return {
    left: `${left}px`,
    top: `${top}px`,
    width: `${Math.abs(endX - startX) || 80}px`,
    height: `${Math.abs(endY - startY) + 80}px`,
  };
}

function connectionPath(from: NodeId, to: NodeId, fromWidth: number, toHeight: number) {
  const startX = nodePositions[from].x + fromWidth;
  const startY = nodePositions[from].y + toHeight / 2;
  const endX = nodePositions[to].x;
  const endY = nodePositions[to].y + toHeight / 2;
  const width = Math.abs(endX - startX) || 80;
  const height = Math.abs(endY - startY) + 80;
  const sx = startX <= endX ? 0 : width;
  const ex = startX <= endX ? width : 0;
  const sy = startY <= endY ? 40 : height - 40;
  const ey = startY <= endY ? height - 40 : 40;
  const c1x = sx + (ex - sx) * 0.45;
  const c2x = sx + (ex - sx) * 0.55;
  return `M ${sx} ${sy} C ${c1x} ${sy}, ${c2x} ${ey}, ${ex} ${ey}`;
}

function startDrag(id: NodeId, event: PointerEvent) {
  const target = event.target as HTMLElement;
  if (target.closest("button,input,select,textarea,label")) return;
  dragging.value = {
    id,
    startX: event.clientX,
    startY: event.clientY,
    originX: nodePositions[id].x,
    originY: nodePositions[id].y,
  };
  window.addEventListener("pointermove", onDrag);
  window.addEventListener("pointerup", stopDrag, { once: true });
}

function onDrag(event: PointerEvent) {
  if (!dragging.value) return;
  const scale = zoom.value || 1;
  const nextX = dragging.value.originX + (event.clientX - dragging.value.startX) / scale;
  const nextY = dragging.value.originY + (event.clientY - dragging.value.startY) / scale;
  nodePositions[dragging.value.id] = {
    x: Math.max(0, Math.round(nextX)),
    y: Math.max(70, Math.round(nextY)),
  };
}

function stopDrag() {
  window.removeEventListener("pointermove", onDrag);
  dragging.value = null;
  saveState();
}

async function generateImagesInBrowser(payload: GeneratePayload): Promise<GeneratedImage[]> {
  if (payload.mode !== "generate") {
    throw new Error("当前是浏览器预览模式。图生图、局部、变体请在 Tauri 桌面客户端中运行。");
  }

  if (payload.provider.kind === "gemini") {
    return generateGeminiInBrowser(payload);
  }

  return generateOpenAiCompatibleInBrowser(payload);
}

async function generateOpenAiCompatibleInBrowser(payload: GeneratePayload): Promise<GeneratedImage[]> {
  const response = await fetch(payload.provider.endpoint.trim(), {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      [payload.provider.authHeader.trim() || "Authorization"]: buildAuthValue(payload.provider),
    },
    body: JSON.stringify({
      model: payload.provider.model.trim(),
      prompt: buildPromptText(payload),
      size: payload.size,
      quality: payload.quality,
      n: payload.count,
      response_format: "b64_json",
      output_format: payload.format,
      background: payload.background,
      moderation: payload.moderation,
      ...(payload.format === "jpeg" || payload.format === "webp" ? { output_compression: payload.compression } : {}),
    }),
  });

  const body = await response.text();
  if (!response.ok) {
    throw new Error(`接口返回 ${response.status}：${compactApiError(body)}`);
  }

  const json = JSON.parse(body);
  const data = Array.isArray(json.data) ? json.data : [];
  const result = data
    .map((item: Record<string, string>) => {
      const mimeType = item.mime_type ?? `image/${payload.format === "jpg" ? "jpeg" : payload.format}`;
      const dataUrl = item.b64_json ? `data:${mimeType};base64,${item.b64_json}` : item.url;
      return dataUrl
        ? {
            dataUrl,
            mimeType,
            model: payload.provider.model,
            provider: payload.provider.name,
          }
        : undefined;
    })
    .filter(Boolean) as GeneratedImage[];

  if (!result.length) throw new Error("响应中没有可显示的图片。");
  return result;
}

async function generateGeminiInBrowser(payload: GeneratePayload): Promise<GeneratedImage[]> {
  const endpoint = payload.provider.endpoint.replace("{model}", payload.provider.model.trim());
  const url = endpoint.includes("?")
    ? `${endpoint}&key=${encodeURIComponent(payload.provider.apiKey.trim())}`
    : `${endpoint}?key=${encodeURIComponent(payload.provider.apiKey.trim())}`;
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      contents: [{ role: "user", parts: [{ text: buildPromptText(payload) }] }],
      generationConfig: {
        responseModalities: ["TEXT", "IMAGE"],
        temperature: payload.temperature,
        imageConfig: {
          aspectRatio: payload.aspectRatio,
          imageSize: payload.imageSize,
        },
        thinkingConfig: {
          thinkingBudget: payload.thinking ? -1 : 0,
        },
      },
      tools: payload.searchGrounding ? [{ googleSearch: {} }] : [],
    }),
  });

  const body = await response.text();
  if (!response.ok) {
    throw new Error(`接口返回 ${response.status}：${compactApiError(body)}`);
  }

  const json = JSON.parse(body);
  const result: GeneratedImage[] = [];
  for (const candidate of json.candidates ?? []) {
    for (const part of candidate.content?.parts ?? []) {
      const inlineData = part.inlineData ?? part.inline_data;
      if (!inlineData?.data) continue;
      const mimeType = inlineData.mimeType ?? inlineData.mime_type ?? "image/png";
      result.push({
        dataUrl: `data:${mimeType};base64,${inlineData.data}`,
        mimeType,
        model: payload.provider.model,
        provider: payload.provider.name,
      });
    }
  }

  if (!result.length) throw new Error("响应中没有 Gemini inlineData 图片。");
  return result;
}

function buildAuthValue(provider: ProviderConfig): string {
  const prefix = provider.authPrefix.trim();
  if (!prefix) return provider.apiKey.trim();
  return `${prefix}${prefix.endsWith(" ") ? "" : " "}${provider.apiKey.trim()}`;
}

function buildPromptText(payload: GeneratePayload): string {
  return [
    `模式：${payload.mode === "generate" ? "文生图" : payload.mode === "edit" ? "图生图" : "变体或局部修改"}`,
    `提示词：${payload.prompt.trim()}`,
    payload.project.trim() ? `项目：${payload.project.trim()}` : "",
    payload.tags.trim() ? `标签：${payload.tags.trim()}` : "",
  ]
    .filter(Boolean)
    .join("\n");
}

function compactApiError(body: string): string {
  try {
    const json = JSON.parse(body);
    return json.error?.message ?? json.message ?? body.slice(0, 500);
  } catch {
    return body.slice(0, 500);
  }
}

function normalizeProviders(value: ProviderConfig[] | undefined): ProviderConfig[] {
  if (!Array.isArray(value)) return defaultProviders.map((provider) => ({ ...provider }));

  return defaultProviders.map((fallback) => {
    const existing = value.find((provider) => provider.id === fallback.id);
    return { ...fallback, ...existing };
  });
}

function optimizePrompt() {
  const base = prompt.value.trim();
  if (!base) return;
  prompt.value = `${base}。请保持主体完整，全身居中，干净浅色背景，服饰层次清晰，线条精致，商业插画品质，避免文字、水印、畸形手指和多余肢体。`;
  saveState();
}

function resetCanvas() {
  zoom.value = 1;
  resetNodePositions();
  saveState();
}

function resetNodePositions() {
  nodePositions.prompt = { x: 82, y: 330 };
  nodePositions.action = { x: 770, y: 118 };
  nodePositions.result = { x: 1328, y: 250 };
}

function clearCanvas() {
  workflowCreated.value = false;
  prompt.value = "";
  images.value = [];
  referenceImages.value = [];
  maskImage.value = undefined;
  errorMessage.value = "";
  activeImageIndex.value = 0;
  zoom.value = 1;
  resetNodePositions();
  status.value = "画布已清空。";
  saveState();
}

function useAsReference() {
  if (!activeImage.value) {
    status.value = "暂无可用结果。";
    return;
  }

  const [meta, data] = activeImage.value.dataUrl.split(",");
  referenceImages.value.push({
    name: `result-${referenceImages.value.length + 1}.${format.value}`,
    mimeType: meta.match(/data:(.*);base64/)?.[1] ?? activeImage.value.mimeType,
    data: data ?? activeImage.value.dataUrl,
  });
  status.value = "当前预览已加入参考图。";
  saveState();
}

function duplicateTemplate(type: ProcessMode) {
  workflowCreated.value = true;
  mode.value = type;
  if (type !== "generate" && activeImage.value && !referenceImages.value.length) {
    referenceImages.value.push(imageToReference(activeImage.value));
    status.value = "当前预览已自动加入参考图。";
  }
  if (!prompt.value && type === "generate") {
    prompt.value = "";
  }
  if (!status.value.includes("参考图")) {
    status.value = type === "generate" ? "文生图模板已创建。" : type === "edit" ? "图生图模板已创建。" : "局部修改模板已创建。";
  }
  showLocalMenu.value = false;
  saveState();
}

async function openActiveImage() {
  if (!activeImage.value) return;
  await runImageCommand("open_image", activeImage.value);
}

async function saveActiveImage() {
  if (!activeImage.value) return;
  const path = await runImageCommand("save_image", activeImage.value);
  status.value = `已保存：${path}`;
}

async function shareActiveImage() {
  if (!activeImage.value) return;
  const path = await runImageCommand("share_image", activeImage.value);
  status.value = `已定位图片：${path}`;
}

async function runImageCommand(command: "open_image" | "save_image" | "share_image", image: GeneratedImage) {
  const payload: ImageCommandPayload = {
    dataUrl: image.dataUrl,
    mimeType: image.mimeType,
    fileName: `myaitoimg-${Date.now()}.${extensionFromMime(image.mimeType)}`,
  };

  if (isTauriRuntime.value) {
    return invoke<string>(command, { payload });
  }

  if (command === "open_image") {
    window.open(image.dataUrl, "_blank", "noopener,noreferrer");
    return "已打开新窗口";
  }

  if (command === "share_image" && navigator.share) {
    await navigator.share({ title: "myaitoimg", url: image.dataUrl });
    return "已调用浏览器分享";
  }

  const link = document.createElement("a");
  link.href = image.dataUrl;
  link.download = payload.fileName;
  document.body.appendChild(link);
  link.click();
  link.remove();
  return payload.fileName;
}

function extensionFromMime(mimeType: string) {
  if (mimeType.includes("jpeg") || mimeType.includes("jpg")) return "jpg";
  if (mimeType.includes("webp")) return "webp";
  return "png";
}

function returnCanvas() {
  showLocalMenu.value = false;
  showSettings.value = false;
}

function openSettings() {
  showSettings.value = true;
  showLocalMenu.value = false;
}

function saveCanvasSnapshot() {
  localStorage.setItem(snapshotKey, localStorage.getItem(storageKey) ?? "");
  status.value = "已保存当前画布快照。";
  showLocalMenu.value = false;
}

function restoreLocalCanvas() {
  const snapshot = localStorage.getItem(snapshotKey);
  if (!snapshot) {
    status.value = "没有本机画布快照。";
    showLocalMenu.value = false;
    return;
  }

  const next = JSON.parse(snapshot);
  selectedProviderId.value = next.selectedProviderId ?? selectedProviderId.value;
  workflowCreated.value = next.workflowCreated ?? false;
  prompt.value = next.prompt ?? "";
  mode.value = next.mode ?? "generate";
  size.value = next.size ?? "1024x1024";
  aspectRatio.value = next.aspectRatio ?? "1:1";
  imageSize.value = next.imageSize ?? "1K";
  quality.value = next.quality ?? "high";
  format.value = next.format ?? "png";
  background.value = next.background ?? "auto";
  moderation.value = next.moderation ?? "auto";
  compression.value = next.compression ?? 100;
  count.value = next.count ?? 1;
  temperature.value = next.temperature ?? 1;
  searchGrounding.value = next.searchGrounding ?? false;
  thinking.value = next.thinking ?? true;
  project.value = next.project ?? "";
  tags.value = next.tags ?? "";
  referenceImages.value = next.referenceImages ?? [];
  maskImage.value = next.maskImage;
  zoom.value = next.zoom ?? 1;
  images.value = next.images ?? [];
  activeImageIndex.value = next.activeImageIndex ?? 0;
  if (next.nodePositions) {
    nodePositions.prompt = next.nodePositions.prompt ?? nodePositions.prompt;
    nodePositions.action = next.nodePositions.action ?? nodePositions.action;
    nodePositions.result = next.nodePositions.result ?? nodePositions.result;
  }
  status.value = "已恢复本机画布。";
  showLocalMenu.value = false;
  saveState();
}

function clearLocalSnapshot() {
  localStorage.removeItem(snapshotKey);
  status.value = "已清除本机快照。";
  showLocalMenu.value = false;
}

function showLocalHistory() {
  status.value = localStorage.getItem(snapshotKey) ? "本机历史：已有 1 个快照。" : "本机历史为空。";
  showLocalMenu.value = false;
}

async function handleReferenceUpload(event: Event) {
  const input = event.target as HTMLInputElement;
  if (!input.files?.length) return;
  const files = Array.from(input.files).slice(0, 14 - referenceImages.value.length);
  const loaded = await Promise.all(files.map(readImageFile));
  referenceImages.value.push(...loaded);
  saveState();
  input.value = "";
}

async function handleMaskUpload(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  maskImage.value = await readImageFile(file);
  saveState();
  input.value = "";
}

function removeReference(index: number) {
  referenceImages.value.splice(index, 1);
  saveState();
}

function readImageFile(file: File): Promise<ReferenceImage> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("读取图片失败"));
    reader.onload = () => {
      const result = String(reader.result);
      const [meta, data] = result.split(",");
      resolve({
        name: file.name,
        mimeType: meta.match(/data:(.*);base64/)?.[1] ?? file.type,
        data,
      });
    };
    reader.readAsDataURL(file);
  });
}
</script>

<template>
  <main class="app-shell">
    <header class="topbar">
      <div>
        <h1>myaitoimg</h1>
        <p>小沨 · xinai.de</p>
      </div>
      <div class="local-mode">
        <button class="mode-pill" type="button" @click="showLocalMenu = !showLocalMenu">
          <span></span>
          本地模式
        </button>
        <nav v-if="showLocalMenu" class="local-menu">
          <button type="button" @click="returnCanvas">返回画布</button>
          <button type="button" @click="showLocalHistory">本地历史</button>
          <button type="button" @click="openSettings">系统设置</button>
          <button type="button" @click="saveCanvasSnapshot">保存当前画布</button>
          <button type="button" @click="restoreLocalCanvas">恢复本机画布</button>
          <button type="button" @click="clearLocalSnapshot">清除本机快照</button>
        </nav>
      </div>
    </header>

    <section class="toolbar left-tools">
      <button type="button" @click="duplicateTemplate('generate')">添加节点</button>
      <button type="button" @click="duplicateTemplate('generate')">文生图模板</button>
      <button type="button" @click="duplicateTemplate('edit')">图生图模板</button>
      <button type="button" @click="duplicateTemplate('variation')">局部修改模板</button>
    </section>

    <section class="toolbar center-tools">
      <button type="button" @click="zoom = Math.min(1.4, zoom + 0.1)">放大</button>
      <button type="button" @click="zoom = Math.max(0.7, zoom - 0.1)">缩小</button>
      <button type="button" @click="resetCanvas">重置</button>
      <button class="danger" type="button" @click="clearCanvas">清空画布</button>
    </section>

    <div class="canvas" :style="{ '--zoom': zoom }">
      <div v-if="!workflowCreated" class="empty-hint">
        <strong>从左侧工具栏新增节点，或右键画布创建工作流</strong>
        <span>推荐流程：提示词 / 参考图 -> 图片处理节点 -> 结果节点。</span>
      </div>

      <article v-if="workflowCreated" class="node prompt-node" :style="nodeStyle('prompt')">
        <header class="node-header drag-handle" @pointerdown="startDrag('prompt', $event)">
          <strong>提示词</strong>
          <span>PROMPT</span>
          <button type="button">功能</button>
        </header>
        <div class="node-body">
          <textarea v-model="prompt" @change="saveState"></textarea>
          <div class="prompt-actions">
            <span>{{ promptCount }} 字</span>
            <button type="button" @click="optimizePrompt">GPT-5.5 优化</button>
          </div>
        </div>
      </article>

      <svg v-if="workflowCreated" class="connection prompt-to-action" :style="connectionStyle('prompt', 'action', 405, 220)" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        <path :d="connectionPath('prompt', 'action', 405, 220)" />
      </svg>

      <article v-if="workflowCreated" class="node action-node" :style="nodeStyle('action')">
        <header class="node-header drag-handle" @pointerdown="startDrag('action', $event)">
          <strong>图片处理</strong>
          <span>ACTION</span>
          <button type="button" @click="showSettings = !showSettings">功能</button>
        </header>
        <div class="tabs">
          <button :class="{ active: mode === 'generate' }" type="button" @click="mode = 'generate'">文生图</button>
          <button :class="{ active: mode === 'edit' }" type="button" @click="mode = 'edit'">图生图</button>
          <button :class="{ active: mode === 'variation' }" type="button" @click="mode = 'variation'">局部</button>
          <button :class="{ active: mode === 'variation' }" type="button" @click="mode = 'variation'">变体</button>
        </div>
        <div class="form-grid">
          <label>
            <span>接口</span>
            <select v-model="selectedProviderId" @change="saveState">
              <option v-for="provider in providers" :key="provider.id" :value="provider.id">{{ provider.name }}</option>
            </select>
          </label>
          <label>
            <span>能力</span>
            <input :value="providerCapability" readonly />
          </label>
          <label>
            <span>模型</span>
            <input v-model="selectedProvider.model" placeholder="gpt-image-2" @change="saveState" />
          </label>
          <label>
            <span>尺寸</span>
            <select v-model="size" @change="saveState">
              <option>1024x1024</option>
              <option>1024x1536</option>
              <option>1536x1024</option>
              <option>auto</option>
            </select>
          </label>
          <label>
            <span>质量</span>
            <select v-model="quality" @change="saveState">
              <option value="high">高</option>
              <option value="medium">中</option>
              <option value="low">低</option>
              <option value="auto">自动</option>
            </select>
          </label>
          <label>
            <span>格式</span>
            <select v-model="format" @change="saveState">
              <option value="png">PNG</option>
              <option value="jpeg">JPEG</option>
              <option value="webp">WEBP</option>
            </select>
          </label>
          <label>
            <span>背景</span>
            <select v-model="background" @change="saveState">
              <option value="auto">自动</option>
              <option value="opaque">不透明</option>
              <option value="transparent">透明</option>
            </select>
          </label>
          <label>
            <span>Gemini 比例</span>
            <select v-model="aspectRatio" @change="saveState">
              <option>1:1</option>
              <option>3:2</option>
              <option>2:3</option>
              <option>4:3</option>
              <option>3:4</option>
              <option>16:9</option>
              <option>9:16</option>
              <option>21:9</option>
            </select>
          </label>
          <label>
            <span>Gemini 清晰度</span>
            <select v-model="imageSize" @change="saveState">
              <option>1K</option>
              <option>2K</option>
              <option>4K</option>
            </select>
          </label>
          <label>
            <span>数量</span>
            <select v-model.number="count" @change="saveState">
              <option :value="1">1</option>
              <option :value="2">2</option>
              <option :value="4">4</option>
            </select>
          </label>
          <label>
            <span>项目</span>
            <input v-model="project" placeholder="例如：新品海报" @change="saveState" />
          </label>
          <label>
            <span>标签</span>
            <input v-model="tags" placeholder="海报, 电商, 写实" @change="saveState" />
          </label>
          <label>
            <span>温度</span>
            <input v-model.number="temperature" type="number" min="0" max="2" step="0.1" @change="saveState" />
          </label>
          <label>
            <span>压缩</span>
            <input v-model.number="compression" type="number" min="0" max="100" step="1" @change="saveState" />
          </label>
        </div>
        <div class="feature-row">
          <label class="check-row">
            <input v-model="searchGrounding" type="checkbox" @change="saveState" />
            <span>Gemini 搜索接地</span>
          </label>
          <label class="check-row">
            <input v-model="thinking" type="checkbox" @change="saveState" />
            <span>Gemini 思考模式</span>
          </label>
          <label class="check-row">
            <span>审核</span>
            <select v-model="moderation" @change="saveState">
              <option value="auto">自动</option>
              <option value="low">低</option>
            </select>
          </label>
        </div>
        <div class="upload-row">
          <label class="file-button">
            参考图 {{ referenceImages.length }}/14
            <input type="file" accept="image/png,image/jpeg,image/webp" multiple @change="handleReferenceUpload" />
          </label>
          <label class="file-button">
            局部蒙版
            <input type="file" accept="image/png" @change="handleMaskUpload" />
          </label>
          <button v-if="maskImage" type="button" @click="maskImage = undefined; saveState()">清除蒙版</button>
        </div>
        <div v-if="referenceImages.length" class="reference-list">
          <button v-for="(image, index) in referenceImages" :key="`${image.name}-${index}`" type="button" @click="removeReference(index)">
            {{ image.name }}
          </button>
        </div>
        <button class="run-button" type="button" :disabled="!canRun || isRunning" @click="runProcess">
          {{ isRunning ? "运行中..." : "运行处理" }}
        </button>
        <div class="progress-box" :class="{ active: isRunning, failed: errorMessage }">
          <div>
            <span>{{ progressLabel }}</span>
            <strong>{{ elapsedText }}</strong>
          </div>
          <progress :value="isRunning ? undefined : errorMessage ? 0 : images.length ? 100 : 0" max="100"></progress>
        </div>
        <div class="status" :class="{ failed: errorMessage }">
          <span>{{ status }}</span>
          <strong>{{ images.length }} / {{ count }} 张</strong>
        </div>
        <p v-if="errorMessage" class="error-message">{{ errorMessage }}</p>
      </article>

      <svg v-if="workflowCreated" class="connection action-to-result" :style="connectionStyle('action', 'result', 470, 540)" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
        <path :d="connectionPath('action', 'result', 470, 540)" />
      </svg>

      <article v-if="workflowCreated" class="node result-node" :style="nodeStyle('result')">
        <header class="node-header drag-handle" @pointerdown="startDrag('result', $event)">
          <strong>结果 {{ images.length ? activeImageIndex + 1 : 1 }}</strong>
          <span>RESULT</span>
          <button type="button">功能</button>
        </header>
        <div class="preview-frame">
          <img v-if="activeImage" :src="activeImage.dataUrl" alt="生成结果" />
          <div v-else class="placeholder">
            <strong>等待生成</strong>
            <span>输入提示词并运行处理</span>
          </div>
        </div>
        <div class="result-actions">
          <button type="button" @click="duplicateTemplate('edit')">编辑</button>
          <button type="button" @click="duplicateTemplate('variation')">局部</button>
          <button type="button" @click="duplicateTemplate('variation')">变体</button>
        </div>
        <div class="result-actions utility-actions">
          <button type="button" :disabled="!activeImage" @click="openActiveImage">打开</button>
          <button type="button" :disabled="!activeImage" @click="saveActiveImage">下载</button>
          <button type="button" :disabled="!activeImage" @click="shareActiveImage">分享</button>
        </div>
        <button class="reference-button" type="button" @click="useAsReference">当前预览转参考图节点</button>
      </article>

      <aside v-if="showSettings" class="settings-panel">
        <header>
          <strong>接口设置</strong>
          <button type="button" @click="showSettings = false">关闭</button>
        </header>
        <label>
          <span>名称</span>
          <input v-model="selectedProvider.name" @change="saveState" />
        </label>
        <label>
          <span>类型</span>
          <select v-model="selectedProvider.kind" @change="onProviderKindChange">
            <option value="openai">OpenAI Images</option>
            <option value="gemini">Gemini GenerateContent</option>
            <option value="custom">OpenAI 兼容自定义</option>
          </select>
        </label>
        <button v-if="selectedProvider.kind !== 'custom'" class="settings-action" type="button" @click="applyOfficialEndpointDefaults">
          使用官方默认接口地址
        </button>
        <label>
          <span>Endpoint</span>
          <input v-model="selectedProvider.endpoint" @change="saveState" />
        </label>
        <label>
          <span>Edit Endpoint</span>
          <input v-model="selectedProvider.editEndpoint" @change="saveState" />
        </label>
        <label>
          <span>API Key</span>
          <input v-model="selectedProvider.apiKey" type="password" placeholder="sk-..." @change="saveState" />
        </label>
        <label>
          <span>模型</span>
          <input v-model="selectedProvider.model" @change="saveState" />
        </label>
        <label>
          <span>鉴权 Header</span>
          <input v-model="selectedProvider.authHeader" placeholder="Authorization" @change="saveState" />
        </label>
        <label>
          <span>鉴权前缀</span>
          <input v-model="selectedProvider.authPrefix" placeholder="Bearer " @change="saveState" />
        </label>
      </aside>
    </div>
  </main>
</template>
