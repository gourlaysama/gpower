use anyhow::*;
use gpower_tweaks::app::GPApplication;

fn main() -> Result<()> {
    gtk::init()?;

    GPApplication::run();

    Ok(())
}
