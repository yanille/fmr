use std::process::Command;

pub fn get_current_branch(path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", path, "branch", "--show-current"])
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}

pub fn is_repo_clean(path: &str) -> bool {
    let output = Command::new("git")
        .args(["-C", path, "status", "--porcelain"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().is_empty()
        }
        _ => true, // Assume clean if we can't check
    }
}

pub fn is_behind_remote(path: &str) -> bool {
    let branch = match get_current_branch(path) {
        Some(b) => b,
        None => return false,
    };

    let output = Command::new("git")
        .args([
            "-C",
            path,
            "rev-list",
            "--left-right",
            "--count",
            &format!("{}...origin/{}", branch, branch),
        ])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(tab_pos) = stdout.find('\t') {
                let behind = &stdout[..tab_pos];
                behind.trim() != "0"
            } else {
                false
            }
        }
        _ => false,
    }
}
