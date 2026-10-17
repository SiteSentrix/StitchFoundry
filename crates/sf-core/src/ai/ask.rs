//! Ask About Embroidery: a question about machine embroidery or about using StitchFoundry, sent to
//! Claude with the conversation so far and, when the user allows it, a short summary of the design
//! open now. As with [`super::ArtRequest`], no network code lives here: the app sends the
//! [`HttpRequest`] that [`AskRequest::http`] builds and hands the answer to [`parse_answer`].

use serde_json::{Value, json};

use super::{API_URL, API_VERSION, ArtError, FALLBACK_BETA, HttpRequest, MODEL, check_status, clean_key, reply_text};

/// The most tokens an answer (thinking included) may use.
pub const MAX_TOKENS: u32 = 8_000;
/// The longest question sent, in characters.
pub const MAX_QUESTION: usize = 2_000;
/// The earlier questions and answers sent along: the latest ones.
pub const MAX_TURNS: usize = 8;
/// The longest earlier answer sent along, in characters (the start of it).
pub const MAX_ANSWER: usize = 4_000;
/// The longest design summary sent, in characters.
pub const MAX_DESIGN: usize = 4_000;

/// A question asked earlier and its answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Turn {
    pub question: String,
    pub answer: String,
}

/// A question to ask.
#[derive(Clone, Debug, PartialEq)]
pub struct AskRequest {
    /// The question, in the user's words.
    pub question: String,
    /// The conversation so far, oldest first.
    pub history: Vec<Turn>,
    /// What the design open now holds, in plain lines (see the app's summary), if the user lets
    /// it go along.
    pub design: Option<String>,
}

/// An answer.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub text: String,
    /// The model that answered (another one when the request fell back).
    pub model: String,
}

/// How StitchFoundry is laid out, so answers can name the right menu items.
const APP_GUIDE: &str = "\
StitchFoundry is a desktop and web app for machine embroidery digitizing, built on a vector \
drawing app. Where things are:
- Embroidery menu: Make a Design… (a step-by-step guide from words, a picture, a shape, appliqué \
or a quilting block), Add Words…, Save for My Machine (writes the machine file to the USB folder \
set in My Machine…), Check Design, Density Map, Compare Designs…
- Embroidery › Create: Convert Artwork to Embroidery, Auto-Digitize Image, Design from Prompt…, \
Redraw Image with Claude…, Digitizing Widget, Cross Stitch, Lettering Kiosk.
- Embroidery › Stitch Type: Run, Bean Run, Backstitch, Stem Stitch, Blanket, Satin, Tatami Fill, \
Satin Fill, Contour Fill, Spiral Fill, Radial Fill, Cross Fill, Meander Fill, Motif Fill, \
Appliqué, Manual Stitches, Sequins, Beads, Rhinestones, Cord, Boring, No Stitches.
- Embroidery › Stitch Direction: Inclination Tool (Shift+I; drag lines across a shape to turn \
its stitches), Use Line as Direction (a drawn line crossing a shape sets its stitch angle), Auto \
Direction…, Reverse Stitch Direction, Swap Satin Sides, Convert to Satin Column.
- Embroidery › Split Lines, Motifs, Add to Design (rings, outlines, borders, laydown, \
buttonholes, echo and background quilting), Shape (remove or fill holes, automatic knife, \
branch, redwork, break apart), Colors and Order (sort by thread colour, match threads to a \
chart, merge colours, auto sequence), Stitches (freeze, regenerate), Special Machines (chain, \
moss, chenille, longarm quilting), Embroidery Fonts, Export Machine File.
- The Embroidery panel (Window › Embroidery) holds the selected object's stitch settings: \
density (spacing), stitch length, underlay, pull compensation, angle, start and end points, \
plus the hoop, fabric, lettering, team names, monograms and design settings such as trims and \
tie-ins. The Stitch Sequence and Stitch List panels order and edit the stitches.
- Watch It Sew (the stitch player) shows the design sewing. File › My Machine… sets the \
machine, its hoops and file format (VP3, PES, JEF, EXP, XXX, DST and more).
- Edit › Undo undoes any step. Help › StitchFoundry Help… (F1) opens the user guide.";

impl AskRequest {
    /// The request with its question trimmed and the conversation cut to the latest turns.
    pub fn new(question: &str, history: &[Turn], design: Option<String>) -> Self {
        let skip = history.len().saturating_sub(MAX_TURNS);
        let history = history.iter().skip(skip).cloned().collect();
        let design = design.map(|d| d.trim().chars().take(MAX_DESIGN).collect::<String>()).filter(|d| !d.is_empty());
        Self { question: question.trim().to_string(), history, design }
    }

    /// The instructions: who the model is helping, how to answer, and the design open now.
    pub fn system_prompt(&self) -> String {
        let mut s = String::from(
            "You answer questions inside StitchFoundry, an embroidery digitizing app, for people \
who sew on home and small-business embroidery machines; many are beginners and not at home with \
computers. Answer what was asked, in plain words, as briefly as the question allows: a few \
sentences, or a short list of steps. Write plain text only: no headings, tables, bold or code, \
and \"- \" for list items. Name StitchFoundry's menu items exactly as listed below when one does \
what the user needs; when you are not sure the app has something, say so rather than inventing \
a menu item. Give measurements in millimetres with inches in brackets. When a problem could \
damage the machine or the fabric, say so first. For questions that have nothing to do with \
embroidery, sewing or this app, say briefly that you can only help with embroidery.\n\n",
        );
        s.push_str(APP_GUIDE);
        if let Some(d) = &self.design {
            s.push_str("\n\nThe design open now (from the app, not from the user):\n");
            s.push_str(d);
        }
        s
    }

    /// The Messages API request body: the earlier turns, then the question.
    pub fn body(&self) -> Value {
        let mut messages: Vec<Value> = Vec::with_capacity(self.history.len() * 2 + 1);
        for t in &self.history {
            let answer: String = t.answer.chars().take(MAX_ANSWER).collect();
            messages.push(json!({ "role": "user", "content": t.question }));
            messages.push(json!({ "role": "assistant", "content": answer }));
        }
        messages.push(json!({ "role": "user", "content": self.question }));
        json!({
            "model": MODEL,
            "max_tokens": MAX_TOKENS,
            "fallbacks": "default",
            "output_config": { "effort": "low" },
            "system": self.system_prompt(),
            "messages": messages,
        })
    }

    /// The HTTP request to send with API key `key`.
    pub fn http(&self, key: &str) -> Result<HttpRequest, ArtError> {
        let key = clean_key(key)?;
        if self.question.is_empty() {
            return Err(ArtError::NoQuestion);
        }
        if self.question.chars().count() > MAX_QUESTION {
            return Err(ArtError::LongQuestion);
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

/// The answer in a reply with HTTP status `status` and body `body`. An answer cut off by its
/// length is kept, ending in "…".
pub fn parse_answer(status: u16, body: &str) -> Result<Answer, ArtError> {
    check_status(status, body)?;
    let v: Value = serde_json::from_str(body).map_err(|_| ArtError::NoAnswer)?;
    let stop = v.get("stop_reason").and_then(Value::as_str);
    if stop == Some("refusal") {
        return Err(ArtError::RefusedAnswer);
    }
    let mut text = reply_text(&v).trim().to_string();
    if text.is_empty() {
        return Err(ArtError::NoAnswer);
    }
    if stop == Some("max_tokens") {
        text.push('…');
    }
    let model = v.get("model").and_then(Value::as_str).unwrap_or(MODEL).to_string();
    Ok(Answer { text, model })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(stop: &str, text: &str) -> String {
        json!({"type": "message", "model": MODEL, "content": [{"type": "thinking", "thinking": "", "signature": "x"}, {"type": "text", "text": text}], "stop_reason": stop}).to_string()
    }

    fn turn(q: &str, a: &str) -> Turn {
        Turn { question: q.into(), answer: a.into() }
    }

    #[test]
    fn the_request_sends_the_conversation_then_the_question() {
        let r = AskRequest::new("  and on towels?  ", &[turn("What density for satin?", "About 0.4 mm.")], None);
        let http = r.http(" sk-ant-test ").unwrap();
        assert_eq!(http.url, API_URL);
        let header = |k: &str| http.headers.iter().find(|h| h.0 == k).map(|h| h.1.as_str());
        assert_eq!(header("x-api-key"), Some("sk-ant-test"));
        assert_eq!(header("anthropic-beta"), Some(FALLBACK_BETA));
        let body: Value = serde_json::from_str(&http.body).unwrap();
        assert_eq!(body["model"], MODEL);
        assert_eq!(body["max_tokens"], MAX_TOKENS);
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["output_config"]["effort"], "low");
        assert_eq!(
            body["messages"],
            json!([
                {"role": "user", "content": "What density for satin?"},
                {"role": "assistant", "content": "About 0.4 mm."},
                {"role": "user", "content": "and on towels?"},
            ])
        );
        let system = body["system"].as_str().unwrap();
        assert!(system.contains("Use Line as Direction") && system.contains("plain text only"), "{system}");
        assert!(!system.contains("The design open now"), "no design sent: {system}");
    }

    #[test]
    fn the_design_goes_in_the_instructions_and_long_conversations_are_cut() {
        let history: Vec<Turn> = (0..12).map(|i| turn(&format!("q{i}"), &"a".repeat(MAX_ANSWER + 50))).collect();
        let r = AskRequest::new("why so many trims?", &history, Some("Stitches: 12,400\nTrims: 31".into()));
        assert_eq!(r.history.len(), MAX_TURNS);
        assert_eq!(r.history[0].question, "q4");
        let body = r.body();
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages.len(), MAX_TURNS * 2 + 1);
        assert_eq!(messages[1]["content"].as_str().unwrap().chars().count(), MAX_ANSWER);
        assert!(body["system"].as_str().unwrap().ends_with("The design open now (from the app, not from the user):\nStitches: 12,400\nTrims: 31"));
        // an empty summary is left out
        assert_eq!(AskRequest::new("q", &[], Some("  ".into())).design, None);
    }

    #[test]
    fn questions_are_checked_before_sending() {
        assert_eq!(AskRequest::new("q", &[], None).http(" "), Err(ArtError::NoKey));
        assert_eq!(AskRequest::new("q", &[], None).http("sk ant"), Err(ArtError::BadKey));
        assert_eq!(AskRequest::new("  ", &[], None).http("k"), Err(ArtError::NoQuestion));
        assert_eq!(AskRequest::new(&"x".repeat(MAX_QUESTION + 1), &[], None).http("k"), Err(ArtError::LongQuestion));
    }

    #[test]
    fn answers_and_their_errors() {
        let a = parse_answer(200, &reply("end_turn", "  Use a tear-away stabilizer.  ")).unwrap();
        assert_eq!(a, Answer { text: "Use a tear-away stabilizer.".into(), model: MODEL.into() });
        assert_eq!(parse_answer(200, &reply("max_tokens", "First,")).unwrap().text, "First,…");
        assert_eq!(parse_answer(200, &reply("max_tokens", "")), Err(ArtError::NoAnswer));
        assert_eq!(parse_answer(200, &reply("refusal", "")), Err(ArtError::RefusedAnswer));
        assert_eq!(parse_answer(200, "not json"), Err(ArtError::NoAnswer));
        assert_eq!(parse_answer(429, ""), Err(ArtError::Busy));
        assert!(matches!(parse_answer(401, r#"{"error": {"message": "invalid x-api-key"}}"#), Err(ArtError::Key(m)) if m == "invalid x-api-key"));
        assert!(ArtError::NoAnswer.retryable());
    }
}
