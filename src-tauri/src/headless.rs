// Headless "sit with Dave" harness. Reproduces the L2 chat boundary
// (run_chat_inference_and_emit) WITHOUT the Tauri webview: it opens the real
// DB, resolves the live persona, assembles the exact same memory partition
// (anchor / canvas / consolidated epochs / recent), and calls the same model
// client. The only thing it drops is the GUI-side pacing/render.
//
// Activated by the DAVE_HEADLESS env var (any value). Reads conversation turns
// from stdin, one per line; prints Dave's reply for each. NON-DESTRUCTIVE —
// it never writes the test turns back to the DB, so it uses Dave's real memory
// as backdrop without polluting his history. Accumulates the running exchange
// in-process so the conversation stays coherent across turns.
//
// Env:
//   DAVE_HEADLESS=1                 activate this mode
//   DAVE_DB=<path to dave.db>       which database (defaults to ./dave.db)
//   assumes llama-server is already serving on 127.0.0.1:8080.

use std::io::BufRead;

use crate::llama_client::{ChatMessage, LlamaClient};

pub fn run() {
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let db_path = std::env::var("DAVE_DB").unwrap_or_else(|_| "dave.db".into());
        let db = match crate::persistence::open(std::path::Path::new(&db_path)) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[headless] cannot open DB {}: {}", db_path, e);
                return;
            }
        };

        let sys = crate::prompts::resolve_active_system_prompt(&db).await;
        let conversation_id = match crate::persistence::latest_conversation_id(&db).await {
            Ok(Some(id)) => id,
            _ => {
                eprintln!("[headless] no conversation in DB {}", db_path);
                return;
            }
        };

        let all_msgs = crate::persistence::load_all_messages(&db, conversation_id)
            .await
            .unwrap_or_default();
        let epochs = crate::persistence::list_active_epochs(&db, conversation_id)
            .await
            .unwrap_or_default();
        let canvas = crate::persistence::get_canvas(&db, conversation_id)
            .await
            .unwrap_or_default();
        let partition = crate::memory_assembler::partition(&all_msgs, &epochs, &canvas);

        eprintln!(
            "[headless] db={} | persona={} chars | memory={} msgs ({} anchor, {} recent, {} epochs, canvas={})",
            db_path,
            sys.chars().count(),
            all_msgs.len(),
            partition.anchor.len(),
            partition.recent.len(),
            epochs.len(),
            if partition.canvas.trim().is_empty() { "empty" } else { "present" },
        );
        eprintln!("[headless] ready — type turns on stdin (one per line), EOF to exit.");

        let client = LlamaClient::new("http://127.0.0.1:8080");
        // The running exchange, appended AFTER Dave's real memory each turn.
        let mut running: Vec<ChatMessage> = Vec::new();

        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let user = match line {
                Ok(l) => l.trim().to_string(),
                Err(_) => break,
            };
            if user.is_empty() {
                continue;
            }

            // Real persona + real memory partition (None = no appended turn),
            // then the in-process running conversation, then this new turn —
            // exactly the shape run_chat_inference_and_emit sends, plus our
            // live thread on top.
            let mut messages = crate::memory_assembler::build_chat_messages(&sys, &partition, None);
            messages.extend(running.iter().cloned());
            messages.push(ChatMessage { role: "user".into(), content: user.clone() });

            match client.chat_stream(messages, |_tok| {}).await {
                Ok(reply) => {
                    println!("YOU:  {}", user);
                    println!("DAVE: {}", reply);
                    println!();
                    running.push(ChatMessage { role: "user".into(), content: user });
                    running.push(ChatMessage { role: "assistant".into(), content: reply });
                }
                Err(e) => eprintln!("[headless] inference error: {}", e),
            }
        }
    });
}
