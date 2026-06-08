use crate::api::{resolve_api_key, validate_page_size, GranolaClient, ListFoldersParams};
use crate::error::CliError;
use crate::output::{print_json, OutputOptions};
use crate::types::Folder;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct FoldersCommand {
    #[command(subcommand)]
    command: FoldersSubcommand,
}

#[derive(Debug, Subcommand)]
enum FoldersSubcommand {
    /// List accessible folders.
    List(ListFoldersCommand),
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

pub async fn handle(
    command: FoldersCommand,
    api_key_override: Option<String>,
    output: &OutputOptions,
) -> Result<(), CliError> {
    match command.command {
        FoldersSubcommand::List(command) => list_folders(command, api_key_override, output).await,
    }
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

    loop {
        let response = client.list_folders(&params).await?;
        for folder in response.folders {
            if command.limit.is_some_and(|limit| folders.len() >= limit) {
                break;
            }
            folders.push(folder);
        }

        if command.limit.is_some_and(|limit| folders.len() >= limit) {
            break;
        }
        if !command.all || !response.has_more {
            break;
        }
        let Some(cursor) = response.cursor else { break };
        params.cursor = Some(cursor);
    }

    if output.is_json() {
        return print_json(&folders, output);
    }

    print_folder_table(&folders);
    Ok(())
}

struct FolderRow<'a> {
    id: &'a str,
    name: &'a str,
    parent_folder_id: &'a str,
}

fn print_folder_table(folders: &[Folder]) {
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
                .map(|row| vec![row.id, row.name, row.parent_folder_id])
                .collect(),
        );
    }
}

fn print_rows(headers: &[&str], rows: Vec<Vec<&str>>) {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in &rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.len());
        }
    }

    print_table_line(&widths);
    print_table_row(headers.iter().copied(), &widths);
    print_table_line(&widths);
    for row in &rows {
        print_table_row(row.iter().copied(), &widths);
    }
    print_table_line(&widths);
}

fn print_table_line(widths: &[usize]) {
    print!("+");
    for width in widths {
        print!("-{:-<width$}-+", "", width = width);
    }
    println!();
}

fn print_table_row<'a>(cells: impl IntoIterator<Item = &'a str>, widths: &[usize]) {
    print!("|");
    for (cell, width) in cells.into_iter().zip(widths) {
        print!(" {cell:<width$} |", width = width);
    }
    println!();
}
