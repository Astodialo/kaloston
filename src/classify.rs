use kalosm::*;
use language::Bert;

pub async fn chaty(input: &String) -> anyhow::Result<()> {
    let bert = Bert::new().await?;

    Ok(())
}
