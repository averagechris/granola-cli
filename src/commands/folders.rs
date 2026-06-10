use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListFoldersParams};
use crate::error::CliError;
use crate::output::{print_json, print_rows, OutputOptions};
use crate::types::Folder;
use clap::{Args, Subcommand};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Args)]
pub struct FoldersCommand {
    #[command(subcommand)]
    command: FoldersSubcommand,
}

#[derive(Debug, Subcommand)]
enum FoldersSubcommand {
    /// List accessible folders.
    List(ListFoldersCommand),
    /// Print accessible folders as a tree.
    Tree(TreeFoldersCommand),
}

#[derive(Debug, Args)]
struct ListFoldersCommand {
    /// Cursor to continue from.
    #[arg(long)]
    cursor: Option<String>,
    /// Page size, capped by the Granola API at 30.
    #[arg(long, default_value_t = 10)]
    page_size: u8,
    /// Fetch all pages.
    #[arg(long)]
    all: bool,
    /// Maximum number of folders to return.
    #[arg(long)]
    limit: Option<usize>,
}

#[derive(Debug, Args)]
struct TreeFoldersCommand {
    /// Page size, capped by the Granola API at 30.
    #[arg(long, default_value_t = 30)]
    page_size: u8,
}

pub async fn handle(
    command: FoldersCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    match command.command {
        FoldersSubcommand::List(command) => list_folders(command, api_key_override, output).await,
        FoldersSubcommand::Tree(command) => tree_folders(command, api_key_override, output).await,
    }
}

async fn tree_folders(
    command: TreeFoldersCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let page_size = validate_page_size(command.page_size)?;
    let mut params = ListFoldersParams {
        cursor: None,
        page_size: Some(page_size),
    };
    let mut folders = Vec::new();

    loop {
        let response = client.list_folders(&params).await?;
        folders.extend(response.folders);
        if !response.has_more {
            break;
        }
        let Some(cursor) = response.cursor else { break };
        params.cursor = Some(cursor);
    }

    let tree = folder_tree(&folders);
    if output.is_json() {
        return print_json(&json!({ "folders": tree, "count": folders.len() }), output);
    }

    print_folder_tree(&folders);
    Ok(())
}

async fn list_folders(
    command: ListFoldersCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    let api_key = resolve_api_key(api_key_override)?;
    let client = GranolaClient::new(api_key)?;
    let page_size = validate_page_size(command.page_size)?;
    let mut params = ListFoldersParams {
        cursor: command.cursor,
        page_size: Some(page_size),
    };
    let mut folders = Vec::new();

    let (has_more, next_cursor) = loop {
        let response = client.list_folders(&params).await?;
        let page_meta = (response.has_more, response.cursor.clone());
        for folder in response.folders {
            if command.limit.is_some_and(|limit| folders.len() >= limit) {
                break;
            }
            folders.push(folder);
        }

        if command.limit.is_some_and(|limit| folders.len() >= limit) {
            break page_meta;
        }
        if !command.all || !response.has_more {
            break page_meta;
        }
        let Some(cursor) = response.cursor else {
            break page_meta;
        };
        params.cursor = Some(cursor);
    };

    if output.is_json() {
        return print_json(
            &json!({
                "folders": folders,
                "count": folders.len(),
                "has_more": has_more,
                "cursor": next_cursor,
                "page_size": page_size,
            }),
            output,
        );
    }

    print_folder_table(&folders, output);
    Ok(())
}

struct FolderRow<'a> {
    id: &'a str,
    name: &'a str,
    parent_folder_id: &'a str,
}

fn print_folder_table(folders: &[Folder], output: &OutputOptions) {
    let rows: Vec<FolderRow<'_>> = folders
        .iter()
        .map(|folder| FolderRow {
            id: &folder.id,
            name: &folder.name,
            parent_folder_id: folder.parent_folder_id.as_deref().unwrap_or(""),
        })
        .collect();

    if rows.is_empty() {
        println!("No folders found");
    } else {
        print_rows(
            &["id", "name", "parent_folder_id"],
            rows.iter()
                .map(|row| {
                    vec![
                        row.id.to_string(),
                        row.name.to_string(),
                        row.parent_folder_id.to_string(),
                    ]
                })
                .collect(),
            output,
        );
    }
}

fn folder_tree(folders: &[Folder]) -> Vec<serde_json::Value> {
    folders
        .iter()
        .map(|folder| {
            json!({
                "id": folder.id,
                "name": folder.name,
                "parent_folder_id": folder.parent_folder_id,
                "path": folder_path(folder, folders),
            })
        })
        .collect()
}

fn folder_path(folder: &Folder, folders: &[Folder]) -> String {
    let by_id: BTreeMap<&str, &Folder> = folders
        .iter()
        .map(|folder| (folder.id.as_str(), folder))
        .collect();
    let mut names = vec![folder.name.as_str()];
    let mut current = folder;
    while let Some(parent) = current
        .parent_folder_id
        .as_deref()
        .and_then(|parent_id| by_id.get(parent_id).copied())
    {
        names.push(parent.name.as_str());
        current = parent;
    }
    names.reverse();
    names.join("/")
}

fn print_folder_tree(folders: &[Folder]) {
    if folders.is_empty() {
        println!("No folders found");
        return;
    }

    let mut children: BTreeMap<Option<&str>, Vec<&Folder>> = BTreeMap::new();
    for folder in folders {
        children
            .entry(folder.parent_folder_id.as_deref())
            .or_default()
            .push(folder);
    }
    for siblings in children.values_mut() {
        siblings.sort_by(|left, right| left.name.cmp(&right.name));
    }

    print_folder_children(None, &children, "");
}

fn print_folder_children(
    parent_id: Option<&str>,
    children: &BTreeMap<Option<&str>, Vec<&Folder>>,
    prefix: &str,
) {
    let Some(siblings) = children.get(&parent_id) else {
        return;
    };
    for (index, folder) in siblings.iter().enumerate() {
        let last = index + 1 == siblings.len();
        let connector = if last { "└── " } else { "├── " };
        println!("{prefix}{connector}{}", folder.name);
        let child_prefix = format!("{prefix}{}", if last { "    " } else { "│   " });
        print_folder_children(Some(&folder.id), children, &child_prefix);
    }
}
