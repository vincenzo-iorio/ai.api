use candid::{CandidType, Deserialize};
use serde::Deserialize as SerdeDeserialize;
use ic_llm::{ChatMessage, AssistantMessage, Model};

#[derive(CandidType, Deserialize, Debug)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(CandidType, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

const SYSTEM_PROMPT: &str = r#"You are a helpful assistant.
Answer user questions clearly and concisely."#;

const MODEL: Model = Model::Llama3_1_8B;

#[derive(SerdeDeserialize, Debug)]
struct IncomingMessage {
    role: String,
    content: String,
}

#[derive(SerdeDeserialize, Debug)]
struct IncomingPayload {
    messages: Vec<IncomingMessage>,
}

#[ic_cdk::update]
async fn chat(messages: Vec<ChatMessage>) -> String {
    ic_cdk::println!("chat() called with {} messages", messages.len());

    let mut all_messages = vec![ChatMessage::System {
        content: SYSTEM_PROMPT.to_string(),
    }];
    all_messages.extend(messages);

    let chat = ic_llm::chat(MODEL).with_messages(all_messages);

    ic_cdk::println!("Sending request to LLM canister…");
    let response = chat.send().await;
    ic_cdk::println!("LLM canister replied: {:?}", response);

    let text = response.message.content.unwrap_or_default();
    ic_cdk::println!("Returning text: {}", text);
    text
}



#[ic_cdk::query]
async fn http_request(req: HttpRequest) -> HttpResponse {
    ic_cdk::println!(
        "http_request: method={} url={} headers={:?}",
        req.method,
        req.url,
        req.headers
    );

    ic_cdk::println!("Raw body: {}", String::from_utf8_lossy(&req.body));

    // Handle preflight CORS
    if req.method.to_uppercase() == "OPTIONS" {
        return HttpResponse {
            status: 204,
            headers: vec![
                ("Access-Control-Allow-Origin".into(), "*".into()),
                (
                    "Access-Control-Allow-Methods".into(),
                    "POST, OPTIONS".into(),
                ),
                (
                    "Access-Control-Allow-Headers".into(),
                    "Content-Type".into(),
                ),
            ],
            body: vec![],
        };
    }

    if req.method.to_uppercase() == "POST" && req.url.starts_with("/chat") {
        match serde_json::from_slice::<IncomingPayload>(&req.body) {
            Ok(payload) => {
                ic_cdk::println!("Parsed JSON: {:?}", payload);

                let mut all_messages = Vec::new();
                for m in payload.messages {
                    match m.role.as_str() {
                        "system" => all_messages.push(ChatMessage::System { content: m.content }),
                        "assistant" => all_messages.push(ChatMessage::Assistant(AssistantMessage {
                            content: Some(m.content),
                            tool_calls: vec![],
                        })),
                        "tool" => all_messages.push(ChatMessage::Tool {
                            content: m.content,
                            tool_call_id: "".into(),
                        }),
                        _ => all_messages.push(ChatMessage::User { content: m.content }),
                    }
                }

                let reply_text = chat(all_messages).await;

                return HttpResponse {
                    status: 200,
                    headers: vec![
                        ("Content-Type".into(), "text/plain".into()),
                        ("Access-Control-Allow-Origin".into(), "*".into()),
                    ],
                    body: reply_text.into_bytes(),
                };
            }
            Err(e) => {
                ic_cdk::println!("JSON parse error: {}", e);
                return HttpResponse {
                    status: 400,
                    headers: vec![
                        ("Content-Type".into(), "text/plain".into()),
                        ("Access-Control-Allow-Origin".into(), "*".into()),
                    ],
                    body: format!("Invalid JSON: {e}").into_bytes(),
                };
            }
        }
    }

    ic_cdk::println!("No matching route for {} {}", req.method, req.url);
    HttpResponse {
        status: 404,
        headers: vec![
            ("Content-Type".into(), "text/plain".into()),
            ("Access-Control-Allow-Origin".into(), "*".into()),
        ],
        body: b"Not Found".to_vec(),
    }
}
