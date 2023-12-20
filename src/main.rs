use anyhow::Result;
use interprocess::os::unix::udsocket::UdStream;
use log::{debug, error};
use regex::Regex;
use std::env;
use std::io::Read;
use std::process::Command;

const UDSOCKET_BUFFER_SIZE: usize = 1024;

fn main() -> Result<()> {
    env_logger::init();

    let his = env::var_os("HYPRLAND_INSTANCE_SIGNATURE")
        .unwrap()
        .into_string()
        .unwrap();
    let stream_path = format!("/tmp/hypr/{his}/.socket2.sock");
    let mut hypr_stream = UdStream::connect(stream_path)?;

    let mut buffer = [0; UDSOCKET_BUFFER_SIZE];
    let mut last_index = 0;

    let mut workspaces = jumpstart_workspaces()?;
    let mut active_workspace: i32 = 1;
    let mut submap = String::new();

    render_workspaces(workspaces.clone(), active_workspace, submap.clone());

    loop {
        let size = hypr_stream.read(&mut buffer[last_index..])?;

        let message = std::str::from_utf8(&buffer[..size]).unwrap();

        let mut copy_over = false;
        let mut re_render = true;

        while let Some(i) = message[last_index..].find('\n') {
            let m = &message[last_index..last_index + i];

            if m.contains('\n') {
                error!("Failed to capture newline correctly. \"{m}\"");
                last_index = 0;
                break;
            }

            let message: &str;
            let args: &str;

            let j: usize = m.find(">>").unwrap();

            message = &m[..j];
            args = &m[(j + 2)..];

            re_render = true;

            match parse_message(message, args)? {
                Event::Workspace(i) => active_workspace = i,
                Event::CreateWorkspace(i) => match workspaces.binary_search(&i) {
                    Ok(j) => error!("Created already existing workspace! {j}"),
                    Err(j) => workspaces.insert(j, i),
                },
                Event::DestroyWorkspace(i) => match workspaces.binary_search(&i) {
                    Ok(j) => {
                        workspaces.remove(j);
                    }
                    Err(j) => error!("Destroyed non-existant workspace: {j}"),
                },
                Event::Submap(map) => submap = map,
                Event::None => re_render = false,
            };

            last_index += i + 1;

            copy_over = true;
        }

        if copy_over {
            copy_over = false;
            buffer.copy_within(last_index.., 0);
            last_index = 0;

            render_workspaces(workspaces.clone(), active_workspace, submap.clone());
        }

        if re_render {
            debug!("{workspaces:?}");
        }
    }
}

enum Event {
    Workspace(i32),
    CreateWorkspace(i32),
    DestroyWorkspace(i32),
    Submap(String),
    None,
}

fn parse_message(message: &str, val: &str) -> Result<Event> {
    match message {
        "workspace" => Ok(Event::Workspace(val.parse()?)),
        "createworkspace" => Ok(Event::CreateWorkspace(val.parse()?)),
        "destroyworkspace" => Ok(Event::DestroyWorkspace(val.parse()?)),
        "submap" => Ok(Event::Submap(String::from(val))),
        _ => Ok(Event::None),
    }
}

fn jumpstart_workspaces() -> Result<Vec<i32>> {
    let workspace_regex = Regex::new(r"\((\d+)\)").unwrap();

    let res: String = String::from_utf8(
        Command::new("hyprctl")
            .args(["workspaces"])
            .output()?
            .stdout,
    )?;

    let mut results = vec![];
    for (_, [cap]) in workspace_regex.captures_iter(&res).map(|c| c.extract()) {
        results.push(cap.parse::<i32>()?);
    }

    results.sort();

    return Ok(results);
}

fn render_workspaces(workspaces: Vec<i32>, active_workspace: i32, submap: String) {
    let workspace_str: String = workspaces
        .iter()
        .zip(std::iter::repeat(active_workspace))
        .map(|(&x, a)| {
            format!(
                "(button :width 10 :onclick \"hyprctl dispatch workspace {x}\" :class \"workspace {}\" \"{}\") ",
                if a == x { "active-workspace" } else { "" },
                map_workspace(x),
            )
        })
        .collect();

    // Render
    println!(
        "(box \
                :class \"workspaces\" \
                :orientation \"h\" \
                :spacing 5 \
                :space-evenly \"false\" \
                {workspace_str} \"{submap}\" )"
    )
}

static ALPHA_CHAR: u32 = 912; // the unicode character Alpha
static SIGMA_CHAR: u32 = ALPHA_CHAR + 19; // the unicode character Sigma

fn map_workspace(workspace: i32) -> String {
    match workspace {
        // I needed to split this because there is a reserved character between rho and sigma.
        i @ 1..=17 => char::from_u32(ALPHA_CHAR + i as u32).unwrap().to_string(),
        i @ 19..=25 => char::from_u32(SIGMA_CHAR + i as u32).unwrap().to_string(),
        i => format!("{}", i),
    }
}
