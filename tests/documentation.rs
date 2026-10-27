use cli_command_kit::Mode;
#[test]
fn usage_skill_covers_live_interface_and_every_help_screen_alerts_agents() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("skills/cron-person");
    let mut text = std::fs::read_to_string(root.join("SKILL.md")).unwrap();
    for topic in [
        "setup",
        "directories",
        "tasks",
        "crontab",
        "hooks",
        "completion",
        "help",
    ] {
        text.push_str(
            &std::fs::read_to_string(root.join(format!("references/{topic}.md"))).unwrap(),
        );
    }
    let mut command = cron_person_cli::cli::command(Mode::Agent);
    let missing =
        cli_command_kit::skill_omissions(&command, &cron_person_cli::cli::help_options(), &text);
    assert!(missing.is_empty(), "{missing:?}");
    command.build();
    fn visit(command: &cli_command_kit::clap::Command, path: Vec<String>) {
        let mut args = path.clone();
        if !path.iter().any(|s| s == "help") {
            args.push("--help".into());
        }
        let error = cron_person_cli::cli::command(Mode::Agent)
            .try_get_matches_from(args)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("ALERT: Agents must read `AGENT=1 cron-person skill`"),
            "{path:?}: {error}"
        );
        for child in command.get_subcommands() {
            let mut next = path.clone();
            next.push(child.get_name().into());
            visit(child, next);
        }
    }
    visit(&command, vec!["cron-person".into()]);
}
