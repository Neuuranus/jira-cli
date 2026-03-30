use crate::api::{ApiError, JiraClient};
use crate::output::OutputConfig;

/// List all available fields, optionally showing only custom fields.
pub async fn list(
    client: &JiraClient,
    out: &OutputConfig,
    custom_only: bool,
) -> Result<(), ApiError> {
    let mut fields = client.list_fields().await?;

    if custom_only {
        fields.retain(|f| f.custom);
    }

    // Sort: system fields first (alphabetically), then custom fields (alphabetically)
    fields.sort_by(|a, b| {
        b.custom
            .cmp(&a.custom)
            .reverse()
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    if out.json {
        out.print_data(
            &serde_json::to_string_pretty(&serde_json::json!({
                "total": fields.len(),
                "fields": fields.iter().map(|f| serde_json::json!({
                    "id": f.id,
                    "name": f.name,
                    "custom": f.custom,
                    "type": f.schema.as_ref().map(|s| &s.field_type),
                })).collect::<Vec<_>>(),
            }))
            .expect("failed to serialize JSON"),
        );
        return Ok(());
    }

    for f in &fields {
        let kind = if f.custom { "custom" } else { "system" };
        let type_str = f
            .schema
            .as_ref()
            .map(|s| s.field_type.as_str())
            .unwrap_or("-");
        println!("{:<30}  {:<25}  {:<8}  {}", f.name, f.id, kind, type_str);
    }
    Ok(())
}
