mod llm;
mod rag;
mod tui;

use color_eyre::Result;
use rag::rag;
use tui::App;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let app_result = App::new().await.run(terminal).await;
    ratatui::restore();
    app_result
}
