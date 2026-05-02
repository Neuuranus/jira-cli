use owo_colors::OwoColorize;

use crate::api::{ApiError, JiraClient};
use crate::output::{OutputConfig, use_color};

pub async fn list(client: &JiraClient, out: &OutputConfig) -> Result<(), ApiError> {
    let projects = client.list_projects().await?;

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "total": projects.len(),
                "projects": projects.iter().map(|p| serde_json::json!({
                    "key": p.key,
                    "name": p.name,
                    "id": p.id,
                    "type": p.project_type,
                })).collect::<Vec<_>>(),
            }))
            .expect("failed to serialize JSON"),
        );
    } else {
        if projects.is_empty() {
            out.print_message("No projects found.");
            return Ok(());
        }

        let color = use_color();
        let key_w = projects
            .iter()
            .map(|p| p.key.len())
            .max()
            .unwrap_or(3)
            .max(3)
            + 2;
        let name_w = projects
            .iter()
            .map(|p| p.name.len())
            .max()
            .unwrap_or(4)
            .max(4)
            + 2;

        let header = format!("{:<key_w$} {:<name_w$} {}", "Key", "Name", "Type");
        if color {
            println!("{}", header.bold());
        } else {
            println!("{header}");
        }

        for p in &projects {
            let ptype = p.project_type.as_deref().unwrap_or("-");
            if color {
                println!(
                    "{} {:<name_w$} {}",
                    format!("{:<key_w$}", p.key).yellow(),
                    p.name,
                    ptype,
                );
            } else {
                println!("{:<key_w$} {:<name_w$} {}", p.key, p.name, ptype);
            }
        }
        out.print_message(&format!("{} projects", projects.len()));
    }
    Ok(())
}

pub async fn components(
    client: &JiraClient,
    out: &OutputConfig,
    project_key: &str,
) -> Result<(), ApiError> {
    let comps = client.list_components(project_key).await?;

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "project": project_key,
                "total": comps.len(),
                "components": comps.iter().map(|c| serde_json::json!({
                    "id": c.id,
                    "name": c.name,
                    "description": c.description,
                })).collect::<Vec<_>>(),
            }))
            .expect("failed to serialize JSON"),
        );
    } else {
        if comps.is_empty() {
            out.print_message(&format!("No components found for project {project_key}."));
            return Ok(());
        }

        let color = use_color();
        let name_w = comps.iter().map(|c| c.name.len()).max().unwrap_or(4).max(4) + 2;
        let id_w = comps.iter().map(|c| c.id.len()).max().unwrap_or(2).max(2) + 2;

        let header = format!("{:<name_w$} {:<id_w$} {}", "Name", "ID", "Description");
        if color {
            println!("{}", header.bold());
        } else {
            println!("{header}");
        }

        for c in &comps {
            let desc = c.description.as_deref().unwrap_or("-");
            if color {
                println!(
                    "{} {:<id_w$} {}",
                    format!("{:<name_w$}", c.name).yellow(),
                    c.id,
                    desc,
                );
            } else {
                println!("{:<name_w$} {:<id_w$} {}", c.name, c.id, desc);
            }
        }
        out.print_message(&format!("{} components", comps.len()));
    }
    Ok(())
}

pub async fn versions(
    client: &JiraClient,
    out: &OutputConfig,
    project_key: &str,
) -> Result<(), ApiError> {
    let vers = client.list_versions(project_key).await?;

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "project": project_key,
                "total": vers.len(),
                "versions": vers.iter().map(|v| serde_json::json!({
                    "id": v.id,
                    "name": v.name,
                    "description": v.description,
                    "released": v.released,
                    "archived": v.archived,
                    "releaseDate": v.release_date,
                })).collect::<Vec<_>>(),
            }))
            .expect("failed to serialize JSON"),
        );
    } else {
        if vers.is_empty() {
            out.print_message(&format!("No versions found for project {project_key}."));
            return Ok(());
        }

        let color = use_color();
        let name_w = vers.iter().map(|v| v.name.len()).max().unwrap_or(4).max(4) + 2;
        let id_w = vers.iter().map(|v| v.id.len()).max().unwrap_or(2).max(2) + 2;

        let header = format!(
            "{:<name_w$} {:<id_w$} {:<10} {}",
            "Name", "ID", "Released", "Release Date"
        );
        if color {
            println!("{}", header.bold());
        } else {
            println!("{header}");
        }

        for v in &vers {
            let released = match v.released {
                Some(true) => "yes",
                Some(false) => "no",
                None => "-",
            };
            let release_date = v.release_date.as_deref().unwrap_or("-");
            if color {
                println!(
                    "{} {:<id_w$} {:<10} {}",
                    format!("{:<name_w$}", v.name).yellow(),
                    v.id,
                    released,
                    release_date,
                );
            } else {
                println!(
                    "{:<name_w$} {:<id_w$} {:<10} {}",
                    v.name, v.id, released, release_date
                );
            }
        }
        out.print_message(&format!("{} versions", vers.len()));
    }
    Ok(())
}

pub async fn show(client: &JiraClient, out: &OutputConfig, key: &str) -> Result<(), ApiError> {
    let project = client.get_project(key).await?;

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "key": project.key,
                "name": project.name,
                "id": project.id,
                "type": project.project_type,
            }))
            .expect("failed to serialize JSON"),
        );
    } else {
        let color = use_color();
        let key_str = if color {
            project.key.yellow().bold().to_string()
        } else {
            project.key.clone()
        };
        println!("{key_str}  {}", project.name);
        println!("  ID:   {}", project.id);
        if let Some(ref t) = project.project_type {
            println!("  Type: {t}");
        }
    }
    Ok(())
}
