use crate::api::{ApiError, JiraClient};
use crate::output::OutputConfig;

/// List sprints, optionally filtered by board name or ID and/or sprint state.
pub async fn list(
    client: &JiraClient,
    out: &OutputConfig,
    board: Option<&str>,
    state: Option<&str>,
) -> Result<(), ApiError> {
    let boards = client.list_boards().await?;

    if boards.is_empty() {
        if out.json {
            out.print_data(r#"{"total":0,"sprints":[]}"#);
        } else {
            out.print_message("No boards found.");
        }
        return Ok(());
    }

    let target_boards: Vec<_> = match board {
        None => boards.iter().collect(),
        Some(b) => {
            // Try numeric ID first, then name substring match.
            if let Ok(id) = b.parse::<u64>() {
                boards.iter().filter(|brd| brd.id == id).collect()
            } else {
                boards
                    .iter()
                    .filter(|brd| brd.name.to_lowercase().contains(&b.to_lowercase()))
                    .collect()
            }
        }
    };

    if target_boards.is_empty() {
        return Err(ApiError::NotFound(format!(
            "No board found matching '{}'",
            board.unwrap_or("")
        )));
    }

    let mut all_sprints = Vec::new();
    for brd in &target_boards {
        let sprints = client.list_sprints(brd.id, state).await?;
        for s in sprints {
            all_sprints.push((brd, s));
        }
    }

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "total": all_sprints.len(),
                "sprints": all_sprints.iter().map(|(b, s)| serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "state": s.state,
                    "boardId": b.id,
                    "boardName": b.name,
                    "startDate": s.start_date,
                    "endDate": s.end_date,
                    "completeDate": s.complete_date,
                })).collect::<Vec<_>>(),
            }))
            .expect("failed to serialize JSON"),
        );
        return Ok(());
    }

    if all_sprints.is_empty() {
        out.print_message("No sprints found.");
        return Ok(());
    }

    for (_, s) in &all_sprints {
        let dates = match s.state.as_str() {
            "active" => format!(
                "{} → {}",
                s.start_date.as_deref().unwrap_or("?"),
                s.end_date.as_deref().unwrap_or("?")
            ),
            "closed" => format!("completed {}", s.complete_date.as_deref().unwrap_or("?")),
            _ => s.end_date.as_deref().unwrap_or("-").to_string(),
        };
        println!("{:>6}  {:<8}  {:<35}  {}", s.id, s.state, s.name, dates);
    }
    Ok(())
}
