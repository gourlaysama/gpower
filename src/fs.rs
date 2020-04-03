use anyhow::*;
use gio::prelude::*;
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
    let mut admin_path = String::with_capacity(path.as_os_str().len() + 8);
    admin_path.push_str("admin://");
    admin_path.push_str(
        path.to_str()
            .ok_or_else(|| anyhow!("Only Unicode paths are supported."))?,
    );

    let file = gio::Vfs::get_default()
        .unwrap()
        .get_file_for_uri(&admin_path)
        .unwrap();

    file.mount_enclosing_volume(
        gio::MountMountFlags::NONE,
        Some(&gio::MountOperation::new()),
        Some(&gio::Cancellable::new()),
        {
            let f = file.clone();
            move |_| {
                let stream = f
                    .replace(
                        None,
                        false,
                        gio::FileCreateFlags::NONE,
                        Some(&gio::Cancellable::new()),
                    )
                    .unwrap();

                func(stream);
            }
        },
    );

    Ok(())
}
