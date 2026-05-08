use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileDocument {
    pub name: String,
    pub path: String,
    pub mime_type: String,
    pub extracted_summary: String,
    pub extracted_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StudentProfile {
    pub full_name: String,
    pub degree_program: String,
    pub semester: String,
    pub about_me: String,
    pub additional_context: String,
    pub top_priorities: Vec<String>,
    pub must_not_ignore: Vec<String>,
    pub important_courses: Vec<String>,
    pub default_help: String,
    pub reminder_style: String,
    pub output_style: String,
    pub documents: Vec<ProfileDocument>,
}

fn storage_dir() -> Result<PathBuf, String> {
    let dir = dirs::home_dir()
        .ok_or_else(|| "Could not resolve home directory".to_string())?
        .join(".imprint")
        .join("student-agent");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn profile_json_path() -> Result<PathBuf, String> {
    Ok(storage_dir()?.join("student_profile.json"))
}

fn user_md_path() -> Result<PathBuf, String> {
    Ok(storage_dir()?.join("user.md"))
}

pub fn load() -> Result<Option<StudentProfile>, String> {
    let path = profile_json_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let profile = serde_json::from_str::<StudentProfile>(&raw).map_err(|e| e.to_string())?;
    Ok(Some(profile))
}

pub fn save(profile: &StudentProfile) -> Result<(), String> {
    let json_path = profile_json_path()?;
    let md_path = user_md_path()?;

    let raw = serde_json::to_string_pretty(profile).map_err(|e| e.to_string())?;
    std::fs::write(json_path, raw).map_err(|e| e.to_string())?;
    std::fs::write(md_path, to_user_md(profile)).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn to_user_md(profile: &StudentProfile) -> String {
    format!(
        "# Student Profile\n\n\
## Identity\n\
- Name: {}\n\
- Degree / Program: {}\n\
- Semester / Year: {}\n\n\
## Personal Details\n\
{}\n\n\
## Extra Context\n\
{}\n\n\
## Top Priorities\n{}\n\n\
## Must-Not-Ignore Signals\n{}\n\n\
## Important Courses\n{}\n\n\
## Default Help\n\
- {}\n\n\
## Reminder Style\n\
- {}\n\n\
## Output Style\n\
- {}\n\n\
## Uploaded Documents\n\
{}\n",
        value_or_placeholder(&profile.full_name),
        value_or_placeholder(&profile.degree_program),
        value_or_placeholder(&profile.semester),
        multiline_or_placeholder(&profile.about_me),
        multiline_or_placeholder(&profile.additional_context),
        render_list(&profile.top_priorities),
        render_list(&profile.must_not_ignore),
        render_list(&profile.important_courses),
        value_or_placeholder(&profile.default_help),
        value_or_placeholder(&profile.reminder_style),
        value_or_placeholder(&profile.output_style),
        render_documents(&profile.documents),
    )
}

fn render_list(items: &[String]) -> String {
    if items.is_empty() {
        "- Not set yet".to_string()
    } else {
        items
            .iter()
            .map(|item| format!("- {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn value_or_placeholder(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "Not set yet".to_string()
    } else {
        trimmed.to_string()
    }
}

fn multiline_or_placeholder(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "Not set yet".to_string()
    } else {
        trimmed.to_string()
    }
}

fn render_documents(documents: &[ProfileDocument]) -> String {
    if documents.is_empty() {
        return "- No documents uploaded yet".to_string();
    }

    documents
        .iter()
        .map(|doc| {
            format!(
                "### {}\n- Source: {}\n- Mime: {}\n- Summary: {}\n\n#### Extracted Text\n{}\n",
                doc.name,
                value_or_placeholder(&doc.path),
                value_or_placeholder(&doc.mime_type),
                value_or_placeholder(&doc.extracted_summary),
                multiline_or_placeholder(&doc.extracted_text),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
