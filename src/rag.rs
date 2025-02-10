use std::time::Instant;

use anyhow::Result;
use kalosm::language::*;
use surrealdb::{
    engine::local::{Db, SurrealKv},
    Surreal,
};

pub async fn rag() -> Result<DocumentTable<Db>> {
    let exists = std::path::Path::new("./db").exists();

    // create database connection
    let db = Surreal::new::<SurrealKv>("./db/temp.db").await?;

    // Select a specific namespace / database
    db.use_ns("kaloston").use_db("kaloston").await?;

    let start_loading_time = Instant::now();
    let bert = Bert::new_for_search().await?;
    println!("Loaded in {:?}", start_loading_time.elapsed());

    // Create a chunker splits the document into chunks to be embedded
    let chunker = SemanticChunker::new()
        .with_target_score(0.65)
        .with_small_chunk_exponent(0.);

    //let url = "https://slatestarcodex.com/2014/07/30/meditations-on-moloch/";
    //let document = Url::parse(&url).unwrap().into_document().await?;

    //let start_time = Instant::now();
    //let chunks = chunker.chunk(&document, &bert).await?;
    //println!("Chunked in {:?}", start_time.elapsed());

    //for (i, chunk) in chunks.iter().enumerate() {
    //    println!(
    //        "Chunk {}:\n {}\n\n",
    //        i,
    //        &document.body()[chunk.byte_range.clone()].trim()
    //    );
    //}
    // Create a table in the surreal database to store the embeddings
    let document_table = db
        .document_table_builder("documents")
        .with_embedding_model(bert)
        .with_chunker(chunker)
        .at("./db/embeddings.db")
        .build::<Document>()
        .await?;

    // If the database is new, add documents to it
    if !exists {
        std::fs::create_dir_all("documents")?;
        let context = ["https://slatestarcodex.com/2014/07/30/meditations-on-moloch/"]
            .iter()
            .map(|url| Url::parse(url).unwrap());

        document_table.add_context(context).await?;
    }

    Ok(document_table)
}
