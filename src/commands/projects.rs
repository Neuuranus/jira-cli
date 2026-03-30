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
        let key_w = projects.iter().map(|p| p.key.len()).max().unwrap_or(3).max(3) + 2;
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
