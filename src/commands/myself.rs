use crate::api::{ApiError, JiraClient};
use crate::output::OutputConfig;

pub async fn show(client: &JiraClient, out: &OutputConfig) -> Result<(), ApiError> {
    let me = client.get_myself().await?;

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "accountId": me.account_id,
                "displayName": me.display_name,
            }))
            .expect("failed to serialize JSON"),
        );
    } else {
        println!("Account ID:   {}", me.account_id);
        println!("Display name: {}", me.display_name);
    }
    Ok(())
}
