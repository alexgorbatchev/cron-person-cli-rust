use cron_person_cli::{CancellationToken, cli};
#[tokio::main]
async fn main() {
    let mode = cli_command_kit::Mode::from_env();
    let matches = match cli::command(mode).try_get_matches() {
        Ok(m) => m,
        Err(e) => {
            let code = if e.use_stderr() { 1 } else { 0 };
            let _ = e.print();
            std::process::exit(code)
        }
    };
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut interrupt =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                    .expect("SIGINT handler");
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("SIGTERM handler");
            tokio::select! {_=interrupt.recv()=>{},_=terminate.recv()=>{}}
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        signal.cancel();
    });
    if let Err(error) = cron_person_cli::execute(
        &matches,
        mode,
        &mut std::io::stdout(),
        &mut std::io::stderr(),
        &cancel,
    )
    .await
    {
        eprintln!(
            "{} {}",
            if mode == cli_command_kit::Mode::Agent {
                "ERR:"
            } else {
                "[ERROR]"
            },
            cli_command_kit::display_message(&error.to_string())
        );
        std::process::exit(cron_person_cli::exit_status(&error));
    }
}
