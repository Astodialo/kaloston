use kalosm::language::*;

pub async fn embed() {
    let model = Bert::new().await.unwrap();

    let embedding = model.embed("Kalosm is a library. It has been really cool so far. It makes running and developing with LLMs a lot easier.").await.unwrap();

    println!("{:?}", embedding.to_vec())
}
