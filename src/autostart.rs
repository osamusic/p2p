use anyhow::Result;

#[cfg(target_os = "linux")]
pub fn setup_autostart() -> Result<()> {
    use std::fs;

    let service_content = r#"[Unit]
Description=P2P Sync Service
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/p2p-sync
Restart=always
RestartSec=10
User=%i

[Install]
WantedBy=multi-user.target
"#;

    let service_path = std::path::Path::new("/etc/systemd/system/p2p-sync@.service");

    if !service_path.exists() {
        fs::write(&service_path, service_content)?;

        std::process::Command::new("systemctl")
            .args(["daemon-reload"])
            .output()?;

        let username = std::env::var("USER").unwrap_or_else(|_| "root".to_string());
        std::process::Command::new("systemctl")
            .args(["enable", &format!("p2p-sync@{username}.service")])
            .output()?;

        tracing::info!("Systemd service installed and enabled");
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn setup_autostart() -> Result<()> {
    use std::fs;

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.p2psync.daemon</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/p2p-sync.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/p2p-sync.err</string>
</dict>
</plist>"#,
        std::env::current_exe()?.display()
    );

    let launch_agents_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
        .join("Library/LaunchAgents");

    fs::create_dir_all(&launch_agents_dir)?;

    let plist_path = launch_agents_dir.join("com.p2psync.daemon.plist");
    fs::write(&plist_path, plist_content)?;

    std::process::Command::new("launchctl")
        .args(&["load", plist_path.to_str().unwrap()])
        .output()?;

    tracing::info!("LaunchAgent installed and loaded");

    Ok(())
}

#[cfg(target_os = "windows")]
pub fn setup_autostart() -> Result<()> {
    use std::os::windows::process::CommandExt;
    use winapi::um::wincon::GetConsoleWindow;
    use winapi::um::winuser::{ShowWindow, SW_HIDE};

    let exe_path = std::env::current_exe()?;
    let startup_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join(r"Microsoft\Windows\Start Menu\Programs\Startup");

    let link_path = startup_dir.join("p2p-sync.lnk");

    let ps_script = format!(
        r#"$WshShell = New-Object -comObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut("{}")
$Shortcut.TargetPath = "{}"
$Shortcut.Save()"#,
        link_path.display(),
        exe_path.display()
    );

    std::process::Command::new("powershell")
        .args(&["-Command", &ps_script])
        .creation_flags(0x08000000)
        .output()?;

    unsafe {
        let window = GetConsoleWindow();
        if !window.is_null() {
            ShowWindow(window, SW_HIDE);
        }
    }

    tracing::info!("Windows startup shortcut created");

    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn setup_autostart() -> Result<()> {
    tracing::warn!("Autostart not implemented for this platform");
    Ok(())
}
