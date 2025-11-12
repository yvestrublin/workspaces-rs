use i3ipc::{event::Event, I3Connection, I3EventListener, Subscription};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, EventKind, Config}; // ajoute Config
use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    sync::mpsc::channel,
    thread,
    time::Duration,
};
use sway::{get_apps, get_tree, Node};

fn main() {
    let icons_path = env::args().nth(1).expect("Usage: workspaces <icons.json>");
    if let Err(e) = listen(&icons_path) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

fn listen(icons_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let icons = Arc::new(Mutex::new(load_icons(icons_path)?));
    let icons_path_buf = PathBuf::from(icons_path);

    // Channel pour la notification de changement de fichier
    let (tx, rx) = channel();

    // Watcher dans un thread séparé
    {
        let tx = tx.clone();
        let icons_path_buf = icons_path_buf.clone();
        thread::spawn(move || {
            let mut watcher: RecommendedWatcher = Watcher::new(
                move |res| { tx.send(res).unwrap(); },
                Config::default(),
            ).unwrap();
            watcher.watch(&icons_path_buf, RecursiveMode::NonRecursive).unwrap();
            loop { thread::sleep(Duration::from_secs(60)); }
        });
    }

    // Thread pour recharger les icônes si le fichier change
    {
        let icons = Arc::clone(&icons);
        let icons_path_buf = icons_path_buf.clone();
        thread::spawn(move || {
            while let Ok(res) = rx.recv() {
                if let Ok(event) = res {
                    use notify::event::ModifyKind;
                    // Recharge si le fichier est modifié, créé ou déplacé
                    let should_reload = match event.kind {
                        EventKind::Modify(ModifyKind::Data(_))
                        | EventKind::Modify(ModifyKind::Any)
                        | EventKind::Create(_)
                        | EventKind::Modify(ModifyKind::Name(_)) => true,
                        _ => false,
                    };
                    if should_reload {
                        if let Ok(new_icons) = load_icons(icons_path_buf.to_str().unwrap()) {
                            let mut icons_lock = icons.lock().unwrap();
                            *icons_lock = new_icons;
                            // eprintln!("Icons reloaded from file.");
                        }
                    }
                }
            }
        });
    }

    let mut conn = I3Connection::connect()?;
    let mut listener = I3EventListener::connect()?;
    let subs = [Subscription::Workspace, Subscription::Window];
    listener.subscribe(&subs)?;

    for event in listener.listen() {
        match event? {
            Event::WindowEvent(_) | Event::WorkspaceEvent(_) => {
                let icons = icons.lock().unwrap();
                update_workspaces(&mut conn, &icons)
            }
            _ => (),
        }
    }
    Ok(())
}

fn load_icons(path: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let data = fs::read_to_string(path)?;
    let parsed = json::parse(&data)?;
    Ok(parsed
        .entries()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect())
}

fn format_workspace_apps(apps: &str, icons: &HashMap<String, String>) -> String {
    apps.lines()
        .map(|line| {
            let app = line.split_once(' ').map(|(a, _)| a).unwrap_or(line);
            icons
                .get(app)
                .map(|icon| format!(" {}", icon))
                .unwrap_or_else(|| " \u{f22d}".to_string())
        })
        .collect()
}

fn rename_workspace(conn: &mut I3Connection, num: &str, name: &str) {
    let cmd = format!("rename workspace number {} to '{}'", num, name);
    if let Err(e) = conn.run_command(&cmd) {
        eprintln!("Failed to rename workspace {num}: {e}");
    }
}

fn update_workspaces(conn: &mut I3Connection, icons: &HashMap<String, String>) {
    for output in get_tree()["nodes"].members() {
        for ws in output["nodes"].members() {
            let num = ws["num"].to_string();
            let apps = get_apps(Node::new(ws));
            if apps.is_empty() {
                rename_workspace(conn, &num, &num);
            } else {
                let name = format!("{}:{} ", num, format_workspace_apps(&apps, icons));
                rename_workspace(conn, &num, &name);
            }
        }
    }
}
