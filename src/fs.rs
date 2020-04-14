use anyhow::*;
use gio::prelude::*;
use gio::IOErrorEnum;
use log::*;
use std::path::Path;

pub async fn write_string_privileged(path: &Path, content: String) -> Result<()> {
    let stream = write_privileged(&path).await?;

    stream.write_all_async_future(content, glib::source::PRIORITY_DEFAULT).await.map_err(|a| a.1)?;

    Ok(())
}

pub async fn write_privileged(path: &Path) -> Result<gio::FileOutputStream> {
    trace!(
        "trying to do a privileged write to {}",
        path.to_str().unwrap_or_default()
    );
    let mut admin_path = String::with_capacity(path.as_os_str().len() + 8);
    admin_path.push_str("admin://");
    admin_path.push_str(
        path.to_str()
            .ok_or_else(|| anyhow!("Only Unicode paths are supported."))?,
    );

    let file = gio::Vfs::get_default()
        .ok_or_else(|| anyhow!("gio error"))?
        .get_file_for_uri(&admin_path)
        .ok_or_else(|| anyhow!("gio file error for {}", admin_path))?;

    let m = file.mount_enclosing_volume_future(
        gio::MountMountFlags::NONE,
        Some(&gio::MountOperation::new()),
    );

    if let Err(e) = m.await {
        match e.kind::<IOErrorEnum>() {
            Some(IOErrorEnum::AlreadyMounted) => {}
            Some(IOErrorEnum::PermissionDenied) => {}
            _ => bail!(e),
        }
    }

    let stream = file.replace_async_future(
        None,
        false,
        gio::FileCreateFlags::NONE,
        glib::source::PRIORITY_DEFAULT,
    ).await?;

    Ok(stream)
}
