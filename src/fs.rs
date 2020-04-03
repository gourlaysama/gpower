use anyhow::*;
use gio::prelude::*;
use log::*;
use std::path::Path;

pub fn write_string_privileged(path: &Path, content: String) -> Result<()> {
    write_privileged(&path, move |stream| {
        stream
            .write(content.as_bytes(), Some(&gio::Cancellable::new()))
            .unwrap();
    })
}

pub fn write_privileged<F>(path: &Path, func: F) -> Result<()>
where
    F: FnOnce(gio::FileOutputStream) + Send + 'static,
{
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

    file.mount_enclosing_volume(
        gio::MountMountFlags::NONE,
        Some(&gio::MountOperation::new()),
        Some(&gio::Cancellable::new()),
        glib::clone!(@strong file => move |_| {
            let stream = file
                .replace(
                    None,
                    false,
                    gio::FileCreateFlags::NONE,
                    Some(&gio::Cancellable::new()),
                ).unwrap();

                func(stream);
        }),
    );

    Ok(())
}
