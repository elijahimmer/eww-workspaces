use anyhow::Result;
use interprocess::os::unix::udsocket::UdStream;
use log::{info, warn};
use std::env;
use std::io::Read;

const UDSOCKET_BUFFER_SIZE: usize = 1024;

fn main() -> Result<()> {
    let his = env::var_os("HYPRLAND_INSTANCE_SIGNATURE")
        .unwrap()
        .into_string()
        .unwrap();
    let stream_path = format!("/tmp/hypr/{his}/.socket2.sock");
    let mut hypr_stream = UdStream::connect(stream_path)?;

    let mut buffer = [0; UDSOCKET_BUFFER_SIZE];
    let mut last_index = 0;

    let mut workspaces: Vec<i32> = vec![1];
    let mut active_workspace: i32 = 1;
    let mut submap = String::new();

    loop {
        let size = hypr_stream.read(&mut buffer[last_index..])?;

        let message = std::str::from_utf8(&buffer[..size]).unwrap();

        let mut copy_over = false;

        while let Some(i) = message[last_index..].find('\n') {
            let m = &message[last_index..last_index + i];

            if m.contains('\n') {
                warn!("Failed to capture newline correctly. \"{m}\"");
                continue;
            }

            let message: &str;
            let args: &str;

            let j: usize = m.find(">>").unwrap();

            message = &m[..j];
            args = &m[j + 2..];

            match handle(message, args)? {
                Event::Workspace(i) => active_workspace = i,
                Event::CreateWorkspace(i) => match workspaces.binary_search(&i) {
                    Ok(j) => warn!("Created already existing workspace! {i}"),
                    Err(j) => workspaces.insert(j, i),
                },
                Event::DestroyWorkspace(i) => match workspaces.binary_search(&i) {
                    Ok(j) => {
                        workspaces.remove(j);
                    }
                    Err(j) => warn!("Destroyed non-existant workspace! {i}"),
                },
                Event::Submap(map) => submap = map,
                Event::None => {}
            };

            last_index += i + 1;

            copy_over = true;
        }

        if copy_over {
            buffer.copy_within(last_index.., 0);
            last_index = 0;
            copy_over = false;
        }

        // Render
        println!()
    }
}

enum Event {
    Workspace(i32),
    CreateWorkspace(i32),
    DestroyWorkspace(i32),
    Submap(String),
    None,
}

fn handle(message: &str, val: &str) -> Result<Event> {
    match message {
        "workspace" => Ok(Event::Workspace(val.parse()?)),
        "createworkspace" => Ok(Event::CreateWorkspace(val.parse()?)),
        "destroyworkspace" => Ok(Event::DestroyWorkspace(val.parse()?)),
        "submap" => Ok(Event::Submap(String::from(val))),
        _ => Ok(Event::None),
    }
}
