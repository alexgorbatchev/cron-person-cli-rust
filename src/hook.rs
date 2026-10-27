use crate::{
    Error, Result, cronrc,
    store::{Status, Store},
};
use std::{io::Write, path::Path};
pub fn generate(shell: &str, executable: &Path, store: &Path) -> Result<String> {
    let quote = |path: &Path| -> Result<String> {
        let text = path
            .to_str()
            .ok_or_else(|| Error::Invalid("shell hook paths must be UTF-8".into()))?;
        let quoted = if shell == "fish" {
            use shell_quote::Quote;
            shell_quote::Fish::quote(text)
        } else {
            shell_quote::Sh::quote_vec(text)
        };
        Ok(String::from_utf8(quoted).expect("UTF-8 shell quoting"))
    };
    let invoke = format!(
        "{} --store {} hook export {shell}",
        quote(executable)?,
        quote(store)?
    );
    match shell {
        "bash" => Ok(format!(
            "_cron_person_hook() {{\n  local previous_exit_status=$?\n  {invoke}\n  return \"$previous_exit_status\"\n}}\nif [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a '* ]]; then\n  _cron_person_registered=\n  for _cron_person_entry in \"${{PROMPT_COMMAND[@]}}\"; do\n    if [[ $_cron_person_entry == _cron_person_hook ]]; then _cron_person_registered=1; fi\n  done\n  if [[ -z $_cron_person_registered ]]; then PROMPT_COMMAND=(_cron_person_hook \"${{PROMPT_COMMAND[@]}}\"); fi\n  unset _cron_person_registered _cron_person_entry\nelse\n  if [[ \";${{PROMPT_COMMAND:-}};\" != *';_cron_person_hook;'* ]]; then PROMPT_COMMAND=\"_cron_person_hook${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}\"; fi\nfi\n"
        )),
        "zsh" => Ok(format!(
            "_cron_person_hook() {{\n  local previous_exit_status=$?\n  {invoke}\n  return $previous_exit_status\n}}\ntypeset -ag precmd_functions chpwd_functions\nif (( ! ${{precmd_functions[(I)_cron_person_hook]}} )); then precmd_functions=(_cron_person_hook $precmd_functions); fi\nif (( ! ${{chpwd_functions[(I)_cron_person_hook]}} )); then chpwd_functions=(_cron_person_hook $chpwd_functions); fi\n"
        )),
        "fish" => Ok(format!(
            "function __cron_person_export_eval --on-event fish_prompt\n  set -l previous_exit_status $status\n  {invoke}\n  return $previous_exit_status\nend\n"
        )),
        _ => Err(Error::Invalid(format!(
            "unsupported shell {shell}; use bash, zsh or fish"
        ))),
    }
}
pub fn export(store: &Store, dir: &Path, agent: bool, stderr: &mut dyn Write) -> Result<()> {
    if let Some(path) = cronrc::discover(dir)? {
        let (status, _) = store.status(path.parent().expect("discovered parent"))?;
        let message = match status {
            Status::Unauthorized => "unauthorized",
            Status::Changed => "modified",
            _ => return Ok(()),
        };
        writeln!(
            stderr,
            "{} {} .cronrc at {}{}",
            if agent {
                "WARN:"
            } else {
                "[WARN] cron-person:"
            },
            message,
            cli_command_kit::display_path(&path),
            if agent {
                ""
            } else {
                " (run 'cron-person dir allow' to approve)"
            }
        )?;
    }
    Ok(())
}
