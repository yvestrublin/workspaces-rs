use i3ipc::{event::Event, I3Connection, I3EventListener, Subscription};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, EventKind, Config}; // ajoute Config
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    sync::mpsc::channel,
    thread,
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
    let filename = icons_path_buf.file_name().unwrap().to_owned();
    
    // Channel pour la notification de changement de fichier
    let (tx, rx) = channel();
    
    // Watcher dans un thread séparé
    {
        let tx = tx.clone();
        let icons_path_buf = icons_path_buf.clone();
        thread::spawn(move || {
            let parent = icons_path_buf
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
            
            let mut watcher: RecommendedWatcher = Watcher::new(
                move |res| { let _ = tx.send(res); }, // évite un panic si rx est fermé
                Config::default(),
            ).unwrap();
            
            // On watch le DOSSIER, pas le fichier
            watcher.watch(&parent, RecursiveMode::NonRecursive).unwrap();
            
            // on garde watcher + filename vivants via un canal séparé, cf. plus bas
            thread::park(); // au lieu du loop sleep(60s), voir optimisations
        });
    }
    
    // Thread pour recharger les icônes si le fichier change
    {
        let icons = Arc::clone(&icons);
        let icons_path_buf = icons_path_buf.clone();
        thread::spawn(move || {
            while let Ok(res) = rx.recv() {
                if let Ok(event) = res {
                    let touches_icons = event.paths.iter().any(|p| {
                        p.file_name().map(|f| f == filename.as_os_str()).unwrap_or(false)
                    });
                    
                    // On accepte tout : Create/Modify/Remove -> couvre les saves atomiques
                    if touches_icons && !matches!(event.kind, EventKind::Access(_)) {
                        if let Ok(new_icons) = load_icons(icons_path_buf.to_str().unwrap()) {
                            *icons.lock().unwrap() = new_icons;
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
