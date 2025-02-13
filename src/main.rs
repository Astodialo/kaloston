mod rag;

use std::{
    future::Future,
    io::Write,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use anyhow::anyhow;
use chrono::Datelike;
use kalosm::language::*;
use rag::rag;
use scraper::{selectable::Selectable, ElementRef, Html, Selector};

#[derive(Schema, Parse, Clone, Debug)]
struct SearchBot {
    search: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model = Llama::builder()
        .with_source(LlamaSource::phi_4())
        .build()
        .await
        .unwrap();

    let system = "We are having a relaxed conversation where we are both trying to learn.\n
    Be accurate with your information and if you don't know say so and don't make something up.\n
    Provide your sources at the end of your response.";

    let mut chat = model.chat().with_system_prompt(system);

    let save_path = std::path::PathBuf::from("./chat.llama");

    println!("Loading history...");
    if let Some(old_session) = std::fs::read(&save_path)
        .ok()
        .and_then(|bytes| LlamaChatSession::from_bytes(&bytes).ok())
    {
        chat = chat.with_session(old_session);
    } else {
    }
    println!("Done!");

    loop {
        let user_q = prompt_input("\n>").unwrap();

        match user_q.to_lowercase().as_str() {
            "bye" => break,
            "exit" => break,
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
                search(&user_q, &model).await?;
                //let prompt = conclude(&user_q).await?;
                //chat(&prompt).to_std_out().await.unwrap();
            }
        }
    }

    let bytes = chat.session().unwrap().to_bytes().unwrap();
    std::fs::write(save_path, bytes).unwrap();

    Ok(())
}

async fn conclude(user_q: &String) -> anyhow::Result<String> {
    // retrieve information from the embeddings db
    println!("searching the db");
    let context = rag()
        .await?
        .search(user_q)
        .with_results(2)
        .await?
        .into_iter()
        .map(|doc| {
            format!(
                "{}\n{}",
                doc.distance,
                doc.record.body()[doc.byte_range.clone()].trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    // Format a prompt with the question and context
    let prompt =
        format!("With this context in mind: \n {context}\n\nAnswer this question: \n{user_q}");

    println!("{}", prompt);

    anyhow::Ok(prompt)
}

async fn search(user_q: &String, model: &Llama) -> anyhow::Result<()> {
    let parser = Arc::new(SearchBot::new_parser());

    let the_now = chrono::offset::Local::now().to_string();

    let (date, time) = the_now.split_at(10);

    let task = format!(
        "You are an agent that creates short sentence searches for a search engine, like duckduckgo.\nBe accurate and up to date with your searches\nIf you need to make a search with the current date, today's is {} and the time is {}.\nConsidering the users question make up to 4 short sentence searches, related to the matter that are going to help you better understand and answer the question with accurate and up to date information",
        date,
        time.trim().split_at(8).0
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
    let search_res_html: Vec<String> = urls
        .iter()
        .map(|url| reqwest::blocking::get(url).unwrap().text().unwrap())
        .collect();

    let search_res_urls: Vec<_> = urls
        .iter()
        .map(|url| {
            let doc = Html::parse_document(url);
            let article = Selector::parse("article#r1-0").unwrap();
            doc.select(&article);
        })
        .collect();

    println!("{:?}", search_res_urls);
    anyhow::Ok(())
}
