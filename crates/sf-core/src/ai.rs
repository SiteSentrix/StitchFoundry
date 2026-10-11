//! Design from Prompt: what StitchFoundry asks an AI model for, and how its reply becomes artwork.
//!
//! The model is Claude, reached through the Messages API with the user's own API key. No network
//! code lives here: the app sends the [`HttpRequest`] that [`ArtRequest::http`] builds with its
//! own client and hands the answer to [`parse_reply`].
//!
//! The art asked for is what digitizes well: filled shapes in a few flat colours, no part narrower
//! than a satin column, stacked back to front (the order they are sewn). The art is placed and
//! given stitches by the engine's `embroidery.placeArt`.

use serde_json::{Value, json};

/// The Messages API endpoint.
pub const API_URL: &str = "https://api.anthropic.com/v1/messages";
/// The API version the request is written against.
pub const API_VERSION: &str = "2023-06-01";
/// The model that draws.
pub const MODEL: &str = "claude-opus-5-5";
/// The beta that lets a request the model declines run again on the model Anthropic recommends
/// (`"fallbacks": "default"`).
pub const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// The most tokens the reply (thinking included) may use.
pub const MAX_TOKENS: u32 = 16_000;
/// The longest description sent, in characters.
pub const MAX_PROMPT: usize = 2_000;
/// The largest artwork accepted, in bytes.
pub const MAX_SVG: usize = 1_000_000;
/// The narrowest part a stitched shape should have, in millimetres.
pub const MIN_FEATURE_MM: f64 = 1.5;
/// Sizes (mm, longest side) and thread colours a request may ask for.
pub const SIZE_RANGE: (f64, f64) = (20.0, 400.0);
pub const COLOR_RANGE: (u32, u32) = (1, 12);

/// Artwork to ask for.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtRequest {
    /// What to draw, in the user's words.
    pub prompt: String,
    /// The stitched size of the longest side, in millimetres.
    pub size_mm: f64,
    /// The most thread colours the art may use.
    pub colors: u32,
}

/// An HTTP POST for the app's client to send.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequest {
    pub url: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: String,
}

/// Artwork the model drew.
#[derive(Clone, Debug, PartialEq)]
pub struct Art {
    /// The SVG document.
    pub svg: String,
    /// The model that drew it (another one when the request fell back).
    pub model: String,
}

/// Why no artwork came back. The messages are for the user; the UI shows them translated.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ArtError {
    #[error("Enter your Anthropic API key first.")]
    NoKey,
    #[error("The API key isn't valid: paste it again from the Anthropic Console.")]
    BadKey,
    #[error("Describe the design first.")]
    NoPrompt,
    #[error("The description is too long: keep it under 2,000 characters.")]
    LongPrompt,
    /// The key was refused (401) or may not use the model (403).
    #[error("Anthropic refused the API key: {0}")]
    Key(String),
    /// The account can't pay for the request (402).
    #[error("Your Anthropic account can't pay for this: {0}")]
    Billing(String),
    /// Too many requests (429) or the service is overloaded (529, 503).
    #[error("The AI service is busy: try again in a minute.")]
    Busy,
    /// The service failed (5xx).
    #[error("The AI service failed (error {0}): try again later.")]
    Service(u16),
    /// Any other refusal of the request, with the API's message.
    #[error("The request failed (error {0}): {1}")]
    Request(u16, String),
    /// The model declined to draw it.
    #[error("The AI declined to draw this: try another description.")]
    Refused,
    /// The reply ran out of room before the artwork was done.
    #[error("The drawing grew too detailed to finish: ask for something simpler.")]
    Truncated,
    /// The reply holds no usable artwork.
    #[error("The reply had no artwork: try again.")]
    NoArt,
    /// The request never got an answer (from the app's client).
    #[error("Couldn't reach the AI service: {0}")]
    Network(String),
}

impl ArtError {
    /// Whether the same request may succeed if sent again shortly.
    pub fn retryable(&self) -> bool {
        matches!(self, Self::Busy | Self::Service(_) | Self::Network(_) | Self::NoArt)
    }
}

/// `key` checked for use in a header: trimmed, non-empty, printable ASCII without spaces.
pub fn clean_key(key: &str) -> Result<&str, ArtError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(ArtError::NoKey);
    }
    if key.len() > 400 || !key.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(ArtError::BadKey);
    }
    Ok(key)
}

impl ArtRequest {
    /// The request with its size and colours brought into range and the description trimmed.
    pub fn new(prompt: &str, size_mm: f64, colors: u32) -> Self {
        let size_mm = if size_mm.is_finite() { size_mm.clamp(SIZE_RANGE.0, SIZE_RANGE.1) } else { 100.0 };
        Self { prompt: prompt.trim().to_string(), size_mm, colors: colors.clamp(COLOR_RANGE.0, COLOR_RANGE.1) }
    }

    /// The instructions that make the model draw stitchable art.
    pub fn system_prompt(&self) -> String {
        let unit = 1000.0 / self.size_mm; // viewBox units per millimetre
        let min = (MIN_FEATURE_MM * unit).ceil();
        let gap = unit.ceil();
        let colors = self.colors;
        let size = self.size_mm.round();
        format!(
            "You draw artwork that will be stitched by an embroidery machine. Reply with one SVG \
document and nothing else: no explanation and no Markdown code fence.\n\
\n\
The artwork is stitched about {size} mm across its longest side, so draw it the way a skilled \
embroidery digitizer would:\n\
- Only filled shapes (path, circle, ellipse, rect, polygon), each with a solid fill=\"#rrggbb\". \
No strokes, gradients, patterns, filters, masks, clip paths, opacity, text, images or fonts. Draw \
outlines and lines as filled shapes too.\n\
- At most {colors} different fill colours, chosen to read well as thread on fabric. No \
background rectangle unless the design is a patch that needs one.\n\
- Bold, flat, simple shapes with clear silhouettes, like a sticker or a patch: no shading, \
hatching, texture or tiny details. Every part is at least {min} units wide (about \
{MIN_FEATURE_MM} mm stitched) and gaps between parts at least {gap} units.\n\
- Shapes go back to front: the first one is sewn first and later ones lie on top of it. Keep to \
about 60 shapes or fewer.\n\
- viewBox=\"0 0 1000 1000\" (another aspect is fine with 1000 on its longest side), absolute \
coordinates, no transforms on the root element."
        )
    }

    /// The Messages API request body.
    pub fn body(&self) -> Value {
        json!({
            "model": MODEL,
            "max_tokens": MAX_TOKENS,
            "fallbacks": "default",
            "output_config": { "effort": "medium" },
            "system": self.system_prompt(),
            "messages": [{ "role": "user", "content": format!("Draw this as embroidery artwork: {}", self.prompt) }],
        })
    }

    /// The HTTP request to send with API key `key`.
    pub fn http(&self, key: &str) -> Result<HttpRequest, ArtError> {
        let key = clean_key(key)?;
        if self.prompt.is_empty() {
            return Err(ArtError::NoPrompt);
        }
        if self.prompt.chars().count() > MAX_PROMPT {
            return Err(ArtError::LongPrompt);
        }
        Ok(HttpRequest {
            url: API_URL,
            headers: vec![
                ("content-type", "application/json".to_string()),
                ("x-api-key", key.to_string()),
                ("anthropic-version", API_VERSION.to_string()),
                ("anthropic-beta", FALLBACK_BETA.to_string()),
            ],
            body: self.body().to_string(),
        })
    }
}

/// The API's own message in an error body (`{"type": "error", "error": {"message": …}}`), else
/// the start of the body.
fn api_message(body: &str) -> String {
    let v: Option<Value> = serde_json::from_str(body).ok();
    if let Some(m) = v.as_ref().and_then(|v| v.pointer("/error/message")).and_then(Value::as_str) {
        return m.to_string();
    }
    let text: String = body.trim().chars().take(200).collect();
    if text.is_empty() { "no details".to_string() } else { text }
}

/// The artwork in a reply with HTTP status `status` and body `body`.
pub fn parse_reply(status: u16, body: &str) -> Result<Art, ArtError> {
    match status {
        200..=299 => {}
        401 | 403 => return Err(ArtError::Key(api_message(body))),
        402 => return Err(ArtError::Billing(api_message(body))),
        429 | 503 | 529 => return Err(ArtError::Busy),
        500..=599 => return Err(ArtError::Service(status)),
        _ => return Err(ArtError::Request(status, api_message(body))),
    }
    let v: Value = serde_json::from_str(body).map_err(|_| ArtError::NoArt)?;
    // The stop reason comes first: a declined or cut-off reply has no finished artwork.
    match v.get("stop_reason").and_then(Value::as_str) {
        Some("refusal") => return Err(ArtError::Refused),
        Some("max_tokens") => return Err(ArtError::Truncated),
        _ => {}
    }
    let text: String = v
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    let svg = extract_svg(&text).ok_or(ArtError::NoArt)?;
    let model = v.get("model").and_then(Value::as_str).unwrap_or(MODEL).to_string();
    Ok(Art { svg: svg.to_string(), model })
}

/// The SVG document in `text`: from its first `<svg` to its last `</svg>` (prose or a code fence
/// around it is left out).
pub fn extract_svg(text: &str) -> Option<&str> {
    let start = text.find("<svg")?;
    let end = text.rfind("</svg>")? + "</svg>".len();
    let svg = text.get(start..end)?;
    (svg.len() <= MAX_SVG).then_some(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000"><circle cx="500" cy="500" r="400" fill="#e07020"/></svg>"##;

    fn reply(stop: &str, blocks: Value) -> String {
        json!({"id": "msg_1", "type": "message", "role": "assistant", "model": MODEL, "content": blocks, "stop_reason": stop}).to_string()
    }

    #[test]
    fn request_names_the_model_effort_fallback_and_art_rules() {
        let r = ArtRequest::new("  a cartoon fox head  ", 80.0, 5);
        assert_eq!(r.prompt, "a cartoon fox head");
        let http = r.http(" sk-ant-test ").unwrap();
        assert_eq!(http.url, API_URL);
        let header = |k: &str| http.headers.iter().find(|h| h.0 == k).map(|h| h.1.as_str());
        assert_eq!(header("x-api-key"), Some("sk-ant-test"));
        assert_eq!(header("anthropic-version"), Some("2023-06-01"));
        assert_eq!(header("anthropic-beta"), Some(FALLBACK_BETA));
        assert_eq!(header("content-type"), Some("application/json"));
        let body: Value = serde_json::from_str(&http.body).unwrap();
        assert_eq!(body["model"], MODEL);
        assert_eq!(body["max_tokens"], MAX_TOKENS);
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["output_config"]["effort"], "medium");
        assert!(body.get("thinking").is_none(), "thinking is always on: not sent");
        assert!(body["messages"][0]["content"].as_str().unwrap().ends_with("a cartoon fox head"));
        let system = body["system"].as_str().unwrap();
        // 80 mm: 12.5 units a millimetre, so 1.5 mm is 19 units and a 1 mm gap 13
        assert!(system.contains("At most 5 different fill colours"), "{system}");
        assert!(system.contains("about 80 mm") && system.contains("at least 19 units") && system.contains("at least 13 units"), "{system}");
    }

    #[test]
    fn requests_are_checked_before_sending() {
        let r = ArtRequest::new("a star", 100.0, 3);
        assert_eq!(r.http("  "), Err(ArtError::NoKey));
        assert_eq!(r.http("sk-ant bad"), Err(ArtError::BadKey));
        assert_eq!(r.http("sk-ant-\r\nx"), Err(ArtError::BadKey));
        assert_eq!(ArtRequest::new(" ", 100.0, 3).http("k"), Err(ArtError::NoPrompt));
        assert_eq!(ArtRequest::new(&"x".repeat(MAX_PROMPT + 1), 100.0, 3).http("k"), Err(ArtError::LongPrompt));
        // out-of-range sizes and colour counts come into range
        let r = ArtRequest::new("a", f64::NAN, 0);
        assert_eq!((r.size_mm, r.colors), (100.0, 1));
        let r = ArtRequest::new("a", 1e9, 99);
        assert_eq!((r.size_mm, r.colors), (SIZE_RANGE.1, COLOR_RANGE.1));
    }

    #[test]
    fn the_svg_is_read_from_text_blocks_around_thinking_and_fences() {
        let body = reply(
            "end_turn",
            json!([
                {"type": "thinking", "thinking": "", "signature": "x"},
                {"type": "text", "text": format!("Here it is:\n```svg\n<?xml version=\"1.0\"?>\n{SVG}\n```")}
            ]),
        );
        let art = parse_reply(200, &body).unwrap();
        assert_eq!(art.svg, SVG);
        assert_eq!(art.model, MODEL);
        // a fallback model that drew it is reported
        let v = json!({"model": "claude-opus-5", "content": [{"type": "text", "text": SVG}], "stop_reason": "end_turn"});
        assert_eq!(parse_reply(200, &v.to_string()).unwrap().model, "claude-opus-5");
    }

    #[test]
    fn stop_reasons_and_missing_art_are_errors() {
        assert_eq!(parse_reply(200, &reply("refusal", json!([]))), Err(ArtError::Refused));
        let cut = reply("max_tokens", json!([{"type": "text", "text": "<svg viewBox=\"0 0 10 10\"><path d=\"M0 0"}]));
        assert_eq!(parse_reply(200, &cut), Err(ArtError::Truncated));
        assert_eq!(parse_reply(200, &reply("end_turn", json!([{"type": "text", "text": "I can't draw that."}]))), Err(ArtError::NoArt));
        assert_eq!(parse_reply(200, "not json"), Err(ArtError::NoArt));
        let huge = format!("<svg>{}</svg>", "x".repeat(MAX_SVG));
        assert_eq!(parse_reply(200, &reply("end_turn", json!([{"type": "text", "text": huge}]))), Err(ArtError::NoArt));
    }

    #[test]
    fn http_errors_carry_the_api_message() {
        let err = |kind: &str, msg: &str| json!({"type": "error", "error": {"type": kind, "message": msg}}).to_string();
        assert_eq!(parse_reply(401, &err("authentication_error", "invalid x-api-key")), Err(ArtError::Key("invalid x-api-key".into())));
        assert_eq!(parse_reply(402, &err("billing_error", "credit balance too low")), Err(ArtError::Billing("credit balance too low".into())));
        assert_eq!(parse_reply(429, &err("rate_limit_error", "slow down")), Err(ArtError::Busy));
        assert_eq!(parse_reply(529, &err("overloaded_error", "overloaded")), Err(ArtError::Busy));
        assert_eq!(parse_reply(500, &err("api_error", "oops")), Err(ArtError::Service(500)));
        assert_eq!(parse_reply(400, &err("invalid_request_error", "bad field")), Err(ArtError::Request(400, "bad field".into())));
        assert_eq!(parse_reply(404, "<html>gone</html>"), Err(ArtError::Request(404, "<html>gone</html>".into())));
        assert_eq!(parse_reply(413, ""), Err(ArtError::Request(413, "no details".into())));
        assert!(ArtError::Busy.retryable() && ArtError::Network("x".into()).retryable() && !ArtError::Refused.retryable());
        assert_eq!(ArtError::Key("invalid x-api-key".into()).to_string(), "Anthropic refused the API key: invalid x-api-key");
    }
}
