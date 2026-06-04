use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error as StdError;
use std::fs;
#[cfg(all(unix, not(target_os = "macos")))]
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::WindowEvent;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratePayload {
    provider: ProviderConfig,
    prompt: String,
    mode: ProcessMode,
    size: String,
    aspect_ratio: String,
    image_size: String,
    quality: String,
    format: String,
    background: String,
    moderation: String,
    compression: u8,
    count: u8,
    temperature: f32,
    search_grounding: bool,
    thinking: bool,
    project: String,
    tags: String,
    reference_images: Vec<ReferenceImage>,
    mask_image: Option<ReferenceImage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderConfig {
    name: String,
    kind: ProviderKind,
    endpoint: String,
    edit_endpoint: String,
    api_key: String,
    model: String,
    auth_header: String,
    auth_prefix: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceImage {
    name: String,
    mime_type: String,
    data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ProviderKind {
    Openai,
    Gemini,
    Custom,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ProcessMode {
    Generate,
    Edit,
    Variation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeneratedImage {
    data_url: String,
    mime_type: String,
    model: String,
    provider: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageCommandPayload {
    data_url: String,
    mime_type: String,
    file_name: String,
}

#[tauri::command]
async fn generate_images(payload: GeneratePayload) -> Result<Vec<GeneratedImage>, String> {
    validate_payload(&payload)?;

    match payload.provider.kind {
        ProviderKind::Gemini => generate_with_gemini(payload).await,
        ProviderKind::Openai | ProviderKind::Custom => {
            generate_with_openai_compatible(payload).await
        }
    }
}

#[tauri::command]
fn open_image(payload: ImageCommandPayload) -> Result<String, String> {
    let path = write_image_file(payload, ImageSaveLocation::Temp)?;
    open_path(&path)?;
    Ok(path)
}

#[tauri::command]
fn save_image(payload: ImageCommandPayload) -> Result<String, String> {
    write_image_file(payload, ImageSaveLocation::Downloads)
}

#[tauri::command]
fn share_image(payload: ImageCommandPayload) -> Result<String, String> {
    let path = write_image_file(payload, ImageSaveLocation::Downloads)?;
    reveal_path(&path)?;
    Ok(path)
}

async fn generate_with_openai_compatible(
    payload: GeneratePayload,
) -> Result<Vec<GeneratedImage>, String> {
    let client = http_client()?;
    let mut headers = HeaderMap::new();
    insert_auth_header(&mut headers, &payload.provider)?;

    if matches!(payload.mode, ProcessMode::Generate) {
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        return generate_openai_json(client, headers, payload).await;
    }

    generate_openai_multipart(client, headers, payload).await
}

async fn generate_openai_json(
    client: reqwest::Client,
    headers: HeaderMap,
    payload: GeneratePayload,
) -> Result<Vec<GeneratedImage>, String> {
    let mut body = json!({
        "model": payload.provider.model,
        "prompt": build_prompt(&payload),
        "size": normalize_size(&payload.size),
        "quality": payload.quality,
        "n": payload.count,
        "response_format": "b64_json",
        "output_format": payload.format,
        "background": payload.background,
        "moderation": payload.moderation
    });

    if payload.format == "jpeg" || payload.format == "webp" {
        body["output_compression"] = json!(payload.compression);
    }

    let response = client
        .post(payload.provider.endpoint.trim())
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("请求失败：{}", request_error(error)))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取响应失败：{error}"))?;

    if !status.is_success() {
        return Err(format!("接口返回 {status}：{}", compact_error(&text)));
    }

    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("响应不是有效 JSON：{error}"))?;
    parse_openai_images(&value, &payload.provider)
}

async fn generate_openai_multipart(
    client: reqwest::Client,
    headers: HeaderMap,
    payload: GeneratePayload,
) -> Result<Vec<GeneratedImage>, String> {
    let endpoint = if payload.provider.edit_endpoint.trim().is_empty() {
        payload.provider.endpoint.trim()
    } else {
        payload.provider.edit_endpoint.trim()
    };

    let mut form = Form::new()
        .text("model", payload.provider.model.clone())
        .text("prompt", build_prompt(&payload))
        .text("size", normalize_size(&payload.size))
        .text("quality", payload.quality.clone())
        .text("n", payload.count.to_string())
        .text("response_format", "b64_json")
        .text("output_format", payload.format.clone())
        .text("background", payload.background.clone());

    if payload.format == "jpeg" || payload.format == "webp" {
        form = form.text("output_compression", payload.compression.to_string());
    }

    for image in &payload.reference_images {
        form = form.part("image", image_part(image)?);
    }

    if let Some(mask) = &payload.mask_image {
        form = form.part("mask", image_part(mask)?);
    }

    let response = client
        .post(endpoint)
        .headers(headers)
        .multipart(form)
        .send()
        .await
        .map_err(|error| format!("请求失败：{}", request_error(error)))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取响应失败：{error}"))?;

    if !status.is_success() {
        return Err(format!("接口返回 {status}：{}", compact_error(&text)));
    }

    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("响应不是有效 JSON：{error}"))?;
    parse_openai_images(&value, &payload.provider)
}

async fn generate_with_gemini(payload: GeneratePayload) -> Result<Vec<GeneratedImage>, String> {
    let client = http_client()?;
    let endpoint = payload
        .provider
        .endpoint
        .replace("{model}", payload.provider.model.trim());
    let url = if endpoint.contains('?') {
        format!("{endpoint}&key={}", payload.provider.api_key.trim())
    } else {
        format!("{endpoint}?key={}", payload.provider.api_key.trim())
    };

    let body = json!({
        "contents": [
            {
                "role": "user",
                "parts": build_gemini_parts(&payload)
            }
        ],
        "generationConfig": {
            "responseModalities": ["TEXT", "IMAGE"],
            "temperature": payload.temperature,
            "imageConfig": {
                "aspectRatio": payload.aspect_ratio,
                "imageSize": payload.image_size
            },
            "thinkingConfig": {
                "thinkingBudget": if payload.thinking { -1 } else { 0 }
            }
        },
        "tools": if payload.search_grounding {
            json!([{ "googleSearch": {} }])
        } else {
            json!([])
        }
    });

    let response = client
        .post(url)
        .header(CONTENT_TYPE, "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|error| format!("请求失败：{}", request_error(error)))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取响应失败：{error}"))?;

    if !status.is_success() {
        return Err(format!("接口返回 {status}：{}", compact_error(&text)));
    }

    let value: Value =
        serde_json::from_str(&text).map_err(|error| format!("响应不是有效 JSON：{error}"))?;
    parse_gemini_images(&value, &payload.provider)
}

fn parse_openai_images(
    value: &Value,
    provider: &ProviderConfig,
) -> Result<Vec<GeneratedImage>, String> {
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "响应缺少 data 数组".to_string())?;

    let images = data
        .iter()
        .filter_map(|item| {
            let mime_type = item
                .get("mime_type")
                .and_then(Value::as_str)
                .unwrap_or("image/png")
                .to_string();

            if let Some(b64) = item.get("b64_json").and_then(Value::as_str) {
                return Some(GeneratedImage {
                    data_url: data_url(&mime_type, b64),
                    mime_type,
                    model: provider.model.clone(),
                    provider: provider.name.clone(),
                });
            }

            item.get("url")
                .and_then(Value::as_str)
                .map(|url| GeneratedImage {
                    data_url: url.to_string(),
                    mime_type,
                    model: provider.model.clone(),
                    provider: provider.name.clone(),
                })
        })
        .collect::<Vec<_>>();

    if images.is_empty() {
        Err("响应中没有可显示的图片".to_string())
    } else {
        Ok(images)
    }
}

fn parse_gemini_images(
    value: &Value,
    provider: &ProviderConfig,
) -> Result<Vec<GeneratedImage>, String> {
    let candidates = value
        .get("candidates")
        .and_then(Value::as_array)
        .ok_or_else(|| "响应缺少 candidates 数组".to_string())?;

    let mut images = Vec::new();

    for candidate in candidates {
        let Some(parts) = candidate
            .get("content")
            .and_then(|content| content.get("parts"))
            .and_then(Value::as_array)
        else {
            continue;
        };

        for part in parts {
            let Some(inline_data) = part.get("inlineData").or_else(|| part.get("inline_data"))
            else {
                continue;
            };
            let Some(b64) = inline_data.get("data").and_then(Value::as_str) else {
                continue;
            };

            let mime_type = inline_data
                .get("mimeType")
                .or_else(|| inline_data.get("mime_type"))
                .and_then(Value::as_str)
                .unwrap_or("image/png")
                .to_string();

            images.push(GeneratedImage {
                data_url: data_url(&mime_type, b64),
                mime_type,
                model: provider.model.clone(),
                provider: provider.name.clone(),
            });
        }
    }

    if images.is_empty() {
        Err("响应中没有 Gemini inlineData 图片".to_string())
    } else {
        Ok(images)
    }
}

fn validate_payload(payload: &GeneratePayload) -> Result<(), String> {
    if payload.prompt.trim().is_empty() {
        return Err("提示词不能为空".to_string());
    }
    if payload.provider.endpoint.trim().is_empty() {
        return Err("Endpoint 不能为空".to_string());
    }
    if payload.provider.model.trim().is_empty() {
        return Err("模型不能为空".to_string());
    }
    if payload.provider.api_key.trim().is_empty() {
        return Err("API Key 不能为空".to_string());
    }
    if payload.count == 0 || payload.count > 4 {
        return Err("数量必须在 1 到 4 之间".to_string());
    }
    if !matches!(payload.mode, ProcessMode::Generate) && payload.reference_images.is_empty() {
        return Err("图生图、局部、变体模式需要至少一张参考图".to_string());
    }
    Ok(())
}

fn build_prompt(payload: &GeneratePayload) -> String {
    let mode = match payload.mode {
        ProcessMode::Generate => "文生图",
        ProcessMode::Edit => "图生图",
        ProcessMode::Variation => "变体或局部修改",
    };

    let mut lines = vec![
        format!("模式：{mode}"),
        format!("提示词：{}", payload.prompt.trim()),
    ];

    if !payload.project.trim().is_empty() {
        lines.push(format!("项目：{}", payload.project.trim()));
    }
    if !payload.tags.trim().is_empty() {
        lines.push(format!("标签：{}", payload.tags.trim()));
    }
    if !payload.reference_images.is_empty() {
        let names = payload
            .reference_images
            .iter()
            .map(|image| image.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("参考图：{names}"));
    }

    lines.join("\n")
}

fn build_gemini_parts(payload: &GeneratePayload) -> Vec<Value> {
    let mut parts = vec![json!({ "text": build_prompt(payload) })];

    for image in &payload.reference_images {
        parts.push(json!({
            "inlineData": {
                "mimeType": image.mime_type,
                "data": image.data
            }
        }));
    }

    if let Some(mask) = &payload.mask_image {
        parts.push(json!({
            "inlineData": {
                "mimeType": mask.mime_type,
                "data": mask.data
            }
        }));
        parts.push(json!({ "text": "上一张图片是局部修改蒙版，请只修改蒙版区域。" }));
    }

    parts
}

fn insert_auth_header(headers: &mut HeaderMap, provider: &ProviderConfig) -> Result<(), String> {
    let header_name = provider.auth_header.trim();
    let prefix = provider.auth_prefix.trim();
    let value = if prefix.is_empty() {
        provider.api_key.trim().to_string()
    } else {
        format!("{prefix} {}", provider.api_key.trim())
    };
    let name: HeaderName = header_name
        .parse()
        .map_err(|_| "鉴权 Header 名称无效".to_string())?;
    let header_value = HeaderValue::from_str(&value).map_err(|_| "API Key 格式无效".to_string())?;
    headers.insert(name, header_value);
    Ok(())
}

fn image_part(image: &ReferenceImage) -> Result<Part, String> {
    let bytes = BASE64
        .decode(image.data.as_bytes())
        .map_err(|_| format!("参考图 {} 不是有效 base64", image.name))?;

    Part::bytes(bytes)
        .file_name(image.name.clone())
        .mime_str(&image.mime_type)
        .map_err(|_| format!("参考图 {} MIME 类型无效", image.name))
}

enum ImageSaveLocation {
    Temp,
    Downloads,
}

fn write_image_file(
    payload: ImageCommandPayload,
    location: ImageSaveLocation,
) -> Result<String, String> {
    let bytes = decode_image_data_url(&payload.data_url)?;
    let directory = match location {
        ImageSaveLocation::Temp => std::env::temp_dir().join("myaitoimg"),
        ImageSaveLocation::Downloads => dirs::download_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())),
    };

    fs::create_dir_all(&directory).map_err(|error| format!("创建目录失败：{error}"))?;
    let file_name = safe_file_name(&payload.file_name, &payload.mime_type);
    let path = directory.join(file_name);
    fs::write(&path, bytes).map_err(|error| format!("写入图片失败：{error}"))?;
    Ok(path.to_string_lossy().to_string())
}

fn decode_image_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    if let Some((_, data)) = data_url.split_once(',') {
        return BASE64
            .decode(data.as_bytes())
            .map_err(|_| "图片 data URL 不是有效 base64".to_string());
    }

    BASE64
        .decode(data_url.as_bytes())
        .map_err(|_| "图片数据不是有效 base64".to_string())
}

fn safe_file_name(file_name: &str, mime_type: &str) -> String {
    let extension = if mime_type.contains("jpeg") || mime_type.contains("jpg") {
        "jpg"
    } else if mime_type.contains("webp") {
        "webp"
    } else {
        "png"
    };
    let stem = file_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let stem = if stem.is_empty() {
        format!("myaitoimg-{millis}")
    } else {
        stem
    };
    if stem.ends_with(&format!(".{extension}")) {
        stem
    } else {
        format!("{stem}.{extension}")
    }
}

fn open_path(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(path)
        .spawn()
        .map_err(|error| format!("打开图片失败：{error}"))?;

    #[cfg(target_os = "windows")]
    Command::new("explorer")
        .arg(path)
        .spawn()
        .map_err(|error| format!("打开图片失败：{error}"))?;

    #[cfg(all(unix, not(target_os = "macos")))]
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|error| format!("打开图片失败：{error}"))?;

    Ok(())
}

fn reveal_path(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn()
        .map_err(|error| format!("定位图片失败：{error}"))?;

    #[cfg(target_os = "windows")]
    Command::new("explorer")
        .arg(format!("/select,{path}"))
        .spawn()
        .map_err(|error| format!("定位图片失败：{error}"))?;

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let directory = Path::new(path).parent().unwrap_or_else(|| Path::new(path));
        Command::new("xdg-open")
            .arg(directory)
            .spawn()
            .map_err(|error| format!("定位图片失败：{error}"))?;
    }

    Ok(())
}

fn normalize_size(size: &str) -> String {
    size.replace('x', "x")
}

fn data_url(mime_type: &str, b64: &str) -> String {
    if b64.starts_with("data:") {
        b64.to_string()
    } else {
        format!("data:{mime_type};base64,{b64}")
    }
}

fn compact_error(text: &str) -> String {
    let parsed: Result<Value, _> = serde_json::from_str(text);
    if let Ok(value) = parsed {
        if let Some(message) = value
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
        {
            return message.to_string();
        }
    }

    text.chars().take(500).collect()
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .user_agent("myaitoimg/0.1.7")
        .build()
        .map_err(|error| format!("创建 HTTP 客户端失败：{}", request_error(error)))
}

fn request_error(error: reqwest::Error) -> String {
    let error = error.without_url();
    let mut message = error.to_string();
    let mut source = error.source();

    while let Some(cause) = source {
        message.push_str("；原因：");
        message.push_str(&cause.to_string());
        source = cause.source();
    }

    message
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = window.minimize() {
                    eprintln!("最小化窗口失败：{error}");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            generate_images,
            open_image,
            save_image,
            share_image
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_b64_images() {
        let raw = BASE64.encode("image");
        let value = json!({ "data": [{ "b64_json": raw, "mime_type": "image/png" }] });
        let provider = ProviderConfig {
            name: "OpenAI".to_string(),
            kind: ProviderKind::Openai,
            endpoint: "https://example.com".to_string(),
            edit_endpoint: "https://example.com/edits".to_string(),
            api_key: "key".to_string(),
            model: "gpt-image-2".to_string(),
            auth_header: "Authorization".to_string(),
            auth_prefix: "Bearer ".to_string(),
        };

        let images = parse_openai_images(&value, &provider).unwrap();
        assert_eq!(images.len(), 1);
        assert!(images[0].data_url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn parses_gemini_inline_data_images() {
        let raw = BASE64.encode("image");
        let value = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "inlineData": {
                            "mimeType": "image/png",
                            "data": raw
                        }
                    }]
                }
            }]
        });
        let provider = ProviderConfig {
            name: "Google Gemini".to_string(),
            kind: ProviderKind::Gemini,
            endpoint: "https://example.com".to_string(),
            edit_endpoint: "https://example.com".to_string(),
            api_key: "key".to_string(),
            model: "gemini-3-pro-image-preview".to_string(),
            auth_header: "x-goog-api-key".to_string(),
            auth_prefix: "".to_string(),
        };

        let images = parse_gemini_images(&value, &provider).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].mime_type, "image/png");
    }
}
