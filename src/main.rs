use anyhow::*;
use gpower_tweaks::app::Application;

fn main() -> Result<()> {
    gtk::init()?;

    let app = Application::new()?;

    app.run();
    
    Ok(())
}