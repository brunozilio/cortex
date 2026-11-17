use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::CommandFactory;
use domain::command::Command;

const LANGS: [&str; 2] = ["en", "pt"];

fn folder(lang: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/docs").join(lang)
}

fn page(lang: &str, slug: &str) -> String {
    std::fs::read_to_string(folder(lang).join(format!("{slug}.md"))).unwrap_or_default()
}

fn slugs(lang: &str) -> BTreeSet<String> {
    std::fs::read_dir(folder(lang))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter_map(|name| name.strip_suffix(".md").map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn missing<'a>(slug: &str, wanted: impl IntoIterator<Item = String> + Clone + 'a) -> Vec<String> {
    LANGS
        .iter()
        .flat_map(|lang| {
            let text = page(lang, slug);
            wanted
                .clone()
                .into_iter()
                .filter(move |item| !text.contains(item.as_str()))
                .map(move |item| format!("{lang}: {item}"))
        })
        .collect()
}

#[test]
fn both_languages_have_the_same_pages_each_with_a_title() {
    let english = slugs("en");
    assert!(english.len() > 20, "the docs are missing: {english:?}");
    assert_eq!(english, slugs("pt"), "every page is written in English and Portuguese");
    for lang in LANGS {
        for slug in &english {
            let text = page(lang, slug);
            assert!(text.starts_with("---\ntitle: "), "{lang}/{slug}.md starts with its title");
        }
    }
}

#[test]
fn every_slash_command_is_documented() {
    let builtin =
        Command::HELP.into_iter().map(|(name, _)| name).chain(Command::ALIASES.into_iter().map(|(name, _)| name));
    let features = super::super::commands().into_iter().flat_map(|(names, _)| names.iter().copied());
    let wanted: Vec<String> =
        builtin.chain(features).filter(|name| *name != "?").map(|name| format!("`/{name}")).collect();
    assert_eq!(missing("slash-commands", wanted), Vec::<String>::new());
}

#[test]
fn every_command_line_flag_and_subcommand_is_documented() {
    let mut cli = crate::Cli::command();
    cli.build();
    let flags = cli.get_arguments().filter_map(|arg| arg.get_long()).map(|long| format!("`--{long}"));
    let subcommands = cli.get_subcommands().map(|sub| format!("{{{{COMMAND}}}} {}", sub.get_name()));
    let wanted: Vec<String> = flags.chain(subcommands).collect();
    assert_eq!(missing("cli-reference", wanted), Vec::<String>::new());
}

#[test]
fn every_feature_flag_is_documented() {
    let wanted: Vec<String> = domain::features::every().map(|flag| format!("`{}`", flag.name)).collect();
    assert_eq!(missing("experimental-features", wanted), Vec::<String>::new());
}
