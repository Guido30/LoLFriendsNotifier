use std::io;
use winresource::WindowsResource;

fn main() -> io::Result<()> {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new().set_icon("src/icons/icon.ico").compile()?;
    }
    Ok(())
}
