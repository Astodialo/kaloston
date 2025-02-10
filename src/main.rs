mod classify;
mod embedding;
mod rag;

use std::{
    future::Future,
    io::Write,
    pin::Pin,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use kalosm::language::*;
use rag::rag;

#[derive(Schema, Parse, Clone, Debug)]
struct Searcher {
    search: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let model = Llama::builder()
        .with_source(LlamaSource::deepseek_r1_distill_qwen_14b())
        .build()
        .await
        .unwrap();

    let system = "You are a researcher and you are having a conversation with a peer. Have a relaxed deminor and explore ideas freely.\n
         Be accurate with your information and if you don't know say so and don't make something up. \n
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
            _ => {
                //search(&user_q, &model).await?;
                let prompt = conclude(&user_q).await?;
                chat(&prompt).to_std_out().await.unwrap();
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
        .with_results(1)
        .await?
        .into_iter()
        .map(|doc| format!("{}", doc.record.body()[doc.byte_range.clone()].trim()))
        .collect::<Vec<_>>()
        .join("\n\n");

    // Format a prompt with the question and context
    let prompt =
        format!("With this context in mind: \n {context}\n\nAnswer this question: \n{user_q}");

    println!("{}", prompt);

    anyhow::Ok(prompt)
}

async fn search(user_q: &String, model: &Llama) -> anyhow::Result<()> {
    let parser = Arc::new(Searcher::new_parser());

    let task = model
        .task(
            "You have access to the internet. Take the users question into account and create\
            a question to input in a search engine, like DuckDuckGo.",
        )
        .with_constraints(parser);

    let searcher: Searcher = task(user_q).await?;
    let words: String = searcher.search.replace(" ", "+");
    let ddg_q: String = "https://duckduckgo.com/?q=".to_owned();
    let link: String = format!("{ddg_q}{words}");
    println!("{}", link);
    let real_visited = Arc::new(AtomicUsize::new(0));
    anyhow::Ok(
        Page::crawl(
            Url::parse(&link).unwrap(),
            BrowserMode::Static,
            move |page: Page| {
                let real_visited = real_visited.clone();
                Box::pin(async move {
                    let visited = real_visited.fetch_add(1, Ordering::SeqCst);

                    println!("{:?}", page.url().domain());
                    let Ok(mut document) = page.html().await else {
                        return CrawlFeedback::follow_none();
                    };

                    println!("{:?}", page.article().await);

                    let original_length = document.html().len();

                    println!("\n{}", original_length);

                    // write the page to disk
                    // let _ = std::fs::create_dir_all("scraped");
                    // if let Ok(mut file) =
                    //     std::fs::File::create(format!("scraped/{visited}.html"))
                    // {
                    //     _ = file.write_all(simplified.as_bytes());
                    // }

                    CrawlFeedback::follow_all()
                }) as Pin<Box<dyn Future<Output = CrawlFeedback>>>
            },
        )
        .await,
    )
}
