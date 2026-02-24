pub struct PlatformConfig {
    pub name: &'static str,
    pub full_name: &'static str,
    pub cli_tool: &'static str,
    pub install_hint: &'static str,
    pub ascii_art: &'static [&'static str],
}
