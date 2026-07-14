use std::process::Command;

/// 创建一个预配置的 git Command，在 Windows 上使用 CREATE_NO_WINDOW
/// 抑制 cmd 窗口闪烁
#[cfg(target_os = "windows")]
pub fn create_git_command() -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = Command::new("git");
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(target_os = "windows"))]
pub fn create_git_command() -> Command {
    Command::new("git")
}
