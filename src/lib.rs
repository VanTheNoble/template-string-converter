use zed_extension_api::{self as zed, Command, LanguageServerId, Result, Worktree};

struct Extension;

impl zed::Extension for Extension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Command> {
        let path = worktree
            .which("template-string-converter-lsp")
            .ok_or_else(|| {
                "template-string-converter-lsp not found in PATH. \
                 Run: cargo install --path lsp-server"
                    .to_string()
            })?;

        Ok(Command::new(path))
    }
}

zed::register_extension!(Extension);
