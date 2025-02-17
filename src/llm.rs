use std::{path::PathBuf, sync::Arc, time::Instant};

use anyhow::anyhow;
use kalosm::language::*;

use crate::rag;

#[derive(Schema, Parse, Clone, Debug)]
pub struct SearchBot {
    search: Vec<String>,
}

pub const SYSTEM: &str = "**Instruction:**  
Adopt the persona of an enthusiastic and engaged anarchist. Your speech should be driven by **elicitive questions** that invite dialogue, encourage reflection, and motivate collective action.
Give accurate information and try not to make stuff up. Answer in English.

**Key Traits to Emphasize:**  
- Always **ask questions** instead of making statements to open up space for conversation.  
- Foster a **spirit of solidarity** and **enthusiasm** when discussing resistance.  
- Seek out and highlight **underdog narratives** in any topic.  
- Encourage collective organizing and **strategic resistance.**  

**Example Conversational Style:**  
- 'Who benefits from this system, and who is left behind?'  
- 'How do you see your role in changing things?'  
- 'What stories of struggle do we often overlook?'  
- 'What would it look like if we organized together?'";

pub struct Agent {
    model: LlamaSource,
    system: &'static str,
    save_path: &'static str,
}

impl Agent {
    pub async fn new() -> Agent {
        Agent {
            model: LlamaSource::deepseek_r1_distill_qwen_14b(),
            system: SYSTEM,
            save_path: "./chat.llama",
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<Chat<Llama>> {
        let model = Llama::builder()
            .with_source(self.model.clone())
            .build()
            .await
            .unwrap();

        let mut chat = model.chat().with_system_prompt(SYSTEM);

        let save_path = PathBuf::from(self.save_path);

        if let Some(old_session) = std::fs::read(&save_path)
            .ok()
            .and_then(|bytes| LlamaChatSession::from_bytes(&bytes).ok())
        {
            chat = chat.with_session(old_session);
        }

        Ok(chat)
    }

    pub async fn chat(&mut self, mut chat: Chat<Llama>, prompt: &str) -> anyhow::Result<()> {
        match prompt.to_lowercase().as_str() {
            "add" => {
                let art_link = prompt_input("\nInsert link please\n>").unwrap();
                println!("Retrieving, chunking and adding...");
                let start_time = Instant::now();
                let document = Url::parse(&art_link).unwrap().into_document().await?;
                let db = rag().await?;
                db.add_context([document]).await?;
                println!("Done in {:?}", start_time.elapsed());
            }
            _ => {
                //search(prompt).await?;
                let prompt = conclude(prompt).await?;
                chat(&prompt).to_std_out().await.unwrap();
            }
        }

        Ok(())
    }
}

async fn conclude(user_q: &str) -> anyhow::Result<String> {
    // retrieve information from the embeddings db
    println!("searching the db");
    let context = rag()
        .await?
        .search(user_q)
        .with_results(2)
        .await?
        .into_iter()
        .map(|doc| {
            if doc.distance > 98.0 {
                format!(
                    "{}\n{}",
                    doc.distance,
                    doc.record.body()[doc.byte_range.clone()].trim()
                )
            } else {
                "".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // Format a prompt with the question and context
    let prompt =
        format!("With this context in mind: \n {context}\n\nAnswer this question: \n{user_q}");

    println!("{}", prompt);

    anyhow::Ok(prompt)
}

async fn search(user_q: &str) -> anyhow::Result<()> {
    let model = Llama::builder()
        .with_source(LlamaSource::phi_3_5_mini_4k_instruct())
        .build()
        .await
        .unwrap();
    let parser = Arc::new(SearchBot::new_parser());

    let the_now = chrono::offset::Local::now().to_string();

    let (date, time) = the_now.split_at(10);

    let task = format!(
        "You are an agent that creates short sentence searches for an article library.
Considering the users question make **up to 4 searches**, related to the matter that are going to help you better understand and answer the question",
    );

    println!("{task}");

    let task = model.task(task).with_constraints(parser);

    let search_bot: SearchBot = task(user_q).await?;
    let searches: Vec<String> = search_bot
        .search
        .iter()
        .map(|search| search.replace(" ", "+"))
        .collect();
    let ddg_q: String = "https://duckduckgo.com/?q=".to_owned();
    let urls: Vec<String> = searches
        .iter()
        .map(|search| format!("{ddg_q}{search}"))
        .collect();
    println!("{:?}", urls);

    anyhow::Ok(())
}
