/// Detect the frontmost (active) macOS application.
/// Returns the app's localized name (e.g. "Slack", "Visual Studio Code", "Mail").
/// Returns None on any failure — detection is best-effort and non-fatal.

#[cfg(target_os = "macos")]
pub fn get_frontmost_app_name() -> Option<String> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let name = app.localizedName()?;
    Some(name.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_app_name() -> Option<String> {
    None
}
