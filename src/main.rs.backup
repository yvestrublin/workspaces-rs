use i3ipc::event::Event;
use i3ipc::I3EventListener;
use i3ipc::Subscription;
use json::JsonValue;
use std::{collections::HashMap, env, fs, process::Command};
use sway::{get_apps, get_workspaces, Node};

const SWAYMSG_BIN: &str = "/usr/bin/swaymsg";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("You must provide a path to an icons json file.");
    }

    let icons: HashMap<String, char> = get_icons(&args[1]);
    let mut listener = I3EventListener::connect().expect("Failed to connect");
    let subs = [Subscription::Workspace, Subscription::Window];

    listener.subscribe(&subs).expect("Failed to subscribe");
    for event in listener.listen() {
        match event {
            Ok(Event::WindowEvent(_w)) => set_workspaces_name(&icons),
            Ok(Event::WorkspaceEvent(_w)) => set_workspaces_name(&icons),
            Err(e) => println!("Error: {}", e),
            _ => unreachable!(),
        }
    }
}

fn get_icons(icons_path: &str) -> HashMap<String, char> {
    json::parse(
        fs::read_to_string(icons_path)
            .expect("Unable to read icons file")
            .as_str(),
    )
    .unwrap()
    .entries()
    .map(|i: (&str, &JsonValue)| (i.0.to_string(), i.1.to_string().chars().next().unwrap()))
    .collect()
}

fn format_workspace_name(apps: &str, icons: &HashMap<String, char>) -> String {
    apps.lines()
        .map(|l: &str| {
            let ls: &str = l.split_once(' ').unwrap_or((l, "")).0;
            if icons.contains_key(ls) {
                " ".to_string() + icons[ls].to_string().as_str()
            } else {
                "  \u{f22d}".to_string()
            }
        })
        .collect::<String>()
}

fn set_workspace_name(num: String, apps: String) {
    Command::new(SWAYMSG_BIN)
        .args([
            "rename",
            "workspace",
            "number",
            num.as_str(),
            "to",
            (num.to_string() + ":" + apps.as_str()).as_str(),
        ])
        .output()
        .expect("Failed to rename workspace");
}

fn clear_workspace_name(num: String) {
    Command::new(SWAYMSG_BIN)
        .args([
            "rename",
            "workspace",
            "number",
            num.as_str(),
            "to",
            num.as_str(),
        ])
        .output()
        .expect("Failed to rename workspace");
}

fn set_workspaces_name(icons: &HashMap<String, char>) {
    get_workspaces().members().for_each(|w: &JsonValue| {
        let apps: String = get_apps(Node::new(w));
        if apps.is_empty() {
            clear_workspace_name(w["num"].to_string())
        } else {
            set_workspace_name(w["num"].to_string(), format_workspace_name(&apps, icons))
        }
    });
}
