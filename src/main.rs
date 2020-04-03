use anyhow::*;
use gpower_tweaks::app::GPApplication;

fn main() -> Result<()> {
    pretty_env_logger::try_init_custom_env("GPOWER_LOG")?;
    gtk::init()?;

    GPApplication::run();

    Ok(())
}
