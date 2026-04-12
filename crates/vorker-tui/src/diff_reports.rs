use std::io;
use std::path::Path;

pub fn copy_to_clipboard(text: &str) -> io::Result<()> {
    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write as _;
        stdin.write_all(text.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(io::Error::other(if stderr.is_empty() {
            "pbcopy failed".to_string()
        } else {
            format!("pbcopy failed: {stderr}")
        }))
    }
}

pub fn render_working_tree_diff(cwd: &Path, max_lines: usize) -> io::Result<String> {
    let status = run_git(cwd, ["status", "--short", "--untracked-files=all"])?;
    let diff = run_git(cwd, ["diff", "--unified=3"])?;
    let staged = run_git(cwd, ["diff", "--cached", "--unified=3"])?;

    let mut sections = Vec::new();
    if !status.trim().is_empty() {
        sections.push(format!("## Git status\n{}", status.trim_end()));
    }
    if !staged.trim().is_empty() {
        sections.push(format!(
            "## Staged diff\n{}",
            truncate_lines(&staged, max_lines)
        ));
    }
    if !diff.trim().is_empty() {
        sections.push(format!(
            "## Unstaged diff\n{}",
            truncate_lines(&diff, max_lines)
        ));
    }

    if sections.is_empty() {
        Ok("Working tree is clean.".to_string())
    } else {
        Ok(sections.join("\n\n"))
    }
}

pub fn render_staged_diff(cwd: &Path, max_lines: usize) -> io::Result<String> {
    let status = run_git(cwd, ["status", "--short", "--untracked-files=all"])?;
    let staged = run_git(cwd, ["diff", "--cached", "--unified=3"])?;

    let mut sections = Vec::new();
    if !status.trim().is_empty() {
        sections.push(format!("## Git status\n{}", status.trim_end()));
    }
    if !staged.trim().is_empty() {
        sections.push(format!(
            "## Staged diff\n{}",
            truncate_lines(&staged, max_lines)
        ));
    }

    if sections.is_empty() {
        Ok("No staged changes.".to_string())
    } else {
        Ok(sections.join("\n\n"))
    }
}

pub fn truncate_lines(text: &str, max_lines: usize) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() <= max_lines {
        text.trim_end().to_string()
    } else {
        let mut truncated = lines[..max_lines].join("\n");
        truncated.push_str("\n\n[diff truncated]");
        truncated
    }
}

fn run_git<I, S>(cwd: &Path, args: I) -> io::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = std::process::Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(io::Error::other(if stderr.is_empty() {
            "git command failed".to_string()
        } else {
            stderr
        }))
    }
}
