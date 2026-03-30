use crate::api::{ApiError, JiraClient};
use crate::output::OutputConfig;

/// Search for users matching a name or email fragment.
pub async fn search(client: &JiraClient, out: &OutputConfig, query: &str) -> Result<(), ApiError> {
    let users = client.search_users(query).await?;

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "total": users.len(),
                "users": users.iter().map(|u| serde_json::json!({
                    "accountId": u.account_id,
                    "displayName": u.display_name,
                    "email": u.email_address,
                })).collect::<Vec<_>>(),
            }))
            .expect("failed to serialize JSON"),
        );
        return Ok(());
    }

    if users.is_empty() {
        out.print_message(&format!("No users found matching '{query}'."));
        return Ok(());
    }

    for u in &users {
        let email = u.email_address.as_deref().unwrap_or("-");
        println!("{:<20} {:<30} {}", u.account_id, u.display_name, email);
    }
    Ok(())
}
