use base64::{engine::general_purpose::STANDARD, Engine as _};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const WIKI_ORIGIN: &str = "https://helldivers.wiki.gg";
const WIKI_PAGE_URL: &str = "https://helldivers.wiki.gg/wiki/Stratagems";
const WIKI_API_URL: &str =
    "https://helldivers.wiki.gg/api.php?action=parse&page=Stratagems&prop=text&format=json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StratagemCatalog {
    pub updated_at_unix: Option<u64>,
    pub source_url: String,
    pub items: Vec<Stratagem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stratagem {
    #[serde(default)]
    pub id: String,
    pub section: String,
    pub category: String,
    pub name: String,
    pub icon_url: String,
    pub command: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct WikiApiResponse {
    parse: WikiApiParse,
}

#[derive(Debug, Deserialize)]
struct WikiApiParse {
    text: WikiApiHtml,
}

#[derive(Debug, Deserialize)]
struct WikiApiHtml {
    #[serde(rename = "*")]
    html: String,
}

impl Default for StratagemCatalog {
    fn default() -> Self {
        Self {
            updated_at_unix: None,
            source_url: WIKI_PAGE_URL.to_string(),
            items: Vec::new(),
        }
    }
}

pub fn load_catalog(app_handle: &AppHandle) -> Result<StratagemCatalog, String> {
    let cache_path = resolve_cache_path(app_handle)?;
    load_catalog_from_path(&cache_path)
}

pub async fn refresh_catalog(app_handle: &AppHandle) -> Result<StratagemCatalog, String> {
    let client = reqwest::Client::builder()
        .user_agent("Hellcall Desktop Stratagem Updater/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let html = fetch_stratagem_page_html(&client).await?;
    let items = parse_stratagems_from_html(&html)?;

    if items.is_empty() {
        return Err("No stratagems were parsed from the wiki response.".to_string());
    }

    let catalog = StratagemCatalog {
        updated_at_unix: Some(current_unix_timestamp()),
        source_url: WIKI_PAGE_URL.to_string(),
        items: items
            .into_iter()
            .map(|item| Stratagem {
                id: compute_stratagem_id(&item.command),
                ..item
            })
            .collect(),
    };

    let cache_path = resolve_cache_path(app_handle)?;
    save_catalog_to_path(&cache_path, &catalog)?;

    Ok(catalog)
}

fn resolve_cache_path(app_handle: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("stratagems.toml"))
}

fn save_catalog_to_path(path: &Path, catalog: &StratagemCatalog) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|parent| !parent.exists()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let content = toml::to_string_pretty(catalog).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

fn load_catalog_from_path(path: &Path) -> Result<StratagemCatalog, String> {
    let default_catalog = StratagemCatalog::default();

    if let Some(parent) = path.parent().filter(|parent| !parent.exists()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    if !path.exists() {
        save_catalog_to_path(path, &default_catalog)?;
        return Ok(default_catalog);
    }

    let file_content = fs::read_to_string(path).map_err(|e| e.to_string())?;

    match toml::from_str::<StratagemCatalog>(&file_content) {
        Ok(mut catalog) => {
            let mut changed = false;
            for item in &mut catalog.items {
                if item.id.is_empty() {
                    item.id = compute_stratagem_id(&item.command);
                    changed = true;
                }
            }

            if changed {
                save_catalog_to_path(path, &catalog)?;
            }

            Ok(catalog)
        }
        Err(error) => {
            log::warn!(
                "Stratagem cache is invalid TOML, resetting cache file: {}",
                error
            );
            let backup_path = path.with_extension("toml.bak");
            let _ = fs::rename(path, &backup_path);
            save_catalog_to_path(path, &default_catalog)?;
            Ok(default_catalog)
        }
    }
}

fn compute_stratagem_id(command: &[String]) -> String {
    STANDARD.encode(command.join(","))
}

async fn fetch_stratagem_page_html(client: &reqwest::Client) -> Result<String, String> {
    match client.get(WIKI_PAGE_URL).send().await {
        Ok(response) if response.status().is_success() => {
            let html = response.text().await.map_err(|e| e.to_string())?;
            if looks_like_cloudflare_challenge(&html) || !html.contains("mw-parser-output") {
                log::warn!("Wiki page response looked like a challenge page, falling back to API.");
            } else {
                return Ok(html);
            }
        }
        Ok(response) => {
            log::warn!(
                "Wiki page returned non-success status {}, falling back to API.",
                response.status()
            );
        }
        Err(error) => {
            log::warn!(
                "Failed to fetch wiki page directly, falling back to API: {}",
                error
            );
        }
    }

    let payload = client
        .get(WIKI_API_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let api_response: WikiApiResponse =
        serde_json::from_str(&payload).map_err(|e| format!("Invalid wiki API response: {}", e))?;

    Ok(api_response.parse.text.html)
}

fn looks_like_cloudflare_challenge(html: &str) -> bool {
    html.contains("__cf_chl")
        || html.contains("Just a moment...")
        || html.contains("challenge-platform")
        || html.contains("Enable JavaScript and cookies to continue")
}

fn parse_stratagems_from_html(html: &str) -> Result<Vec<Stratagem>, String> {
    let document = Html::parse_document(html);
    let container_selector =
        Selector::parse(".mw-parser-output").map_err(|e| format!("Invalid selector: {}", e))?;
    let content_root = document
        .select(&container_selector)
        .next()
        .ok_or_else(|| "Wiki response did not include .mw-parser-output".to_string())?;

    let mut items = Vec::new();
    let mut current_section = String::new();
    let mut current_category = String::new();

    for child in content_root.children() {
        let Some(element) = ElementRef::wrap(child) else {
            continue;
        };

        match element.value().name() {
            "h2" => {
                current_section = extract_heading_text(&element);
                current_category.clear();
            }
            "h3" => {
                current_category = extract_heading_text(&element);
            }
            "table"
                if element
                    .value()
                    .classes()
                    .any(|class_name| class_name == "wikitable") =>
            {
                if matches!(
                    current_section.as_str(),
                    "Current Stratagems" | "Mission Stratagems"
                ) {
                    items.extend(parse_stratagem_table(
                        &element,
                        &current_section,
                        &current_category,
                    ));
                }
            }
            _ => {}
        }
    }

    Ok(items)
}

fn parse_stratagem_table(table: &ElementRef<'_>, section: &str, category: &str) -> Vec<Stratagem> {
    let row_selector = Selector::parse("tr").expect("valid row selector");
    let rows = table.select(&row_selector).collect::<Vec<_>>();
    if rows.is_empty() {
        return Vec::new();
    }

    let header_cells = direct_cells(&rows[0]);
    let icon_index = find_column_index(&header_cells, "Icon").unwrap_or(0);
    let name_index = find_column_index(&header_cells, "Name").unwrap_or(1);
    let command_index = find_column_index(&header_cells, "Stratagem Code").unwrap_or(2);
    let required_index = icon_index.max(name_index).max(command_index);

    rows.iter()
        .skip(1)
        .filter_map(|row| {
            let cells = direct_cells(row);
            if cells.len() <= required_index {
                return None;
            }

            let name = extract_name(&cells[name_index]);
            let command = extract_command(&cells[command_index]);

            if name.is_empty() || command.is_empty() {
                return None;
            }

            Some(Stratagem {
                id: String::new(),
                section: section.to_string(),
                category: if category.is_empty() {
                    section.to_string()
                } else {
                    category.to_string()
                },
                name,
                icon_url: extract_image_url(&cells[icon_index]).unwrap_or_default(),
                command,
            })
        })
        .collect()
}

fn direct_cells<'a>(row: &'a ElementRef<'a>) -> Vec<ElementRef<'a>> {
    row.children()
        .filter_map(ElementRef::wrap)
        .filter(|cell| matches!(cell.value().name(), "th" | "td"))
        .collect()
}

fn find_column_index(cells: &[ElementRef<'_>], header_name: &str) -> Option<usize> {
    cells.iter().position(|cell| {
        normalize_whitespace(&cell.text().collect::<Vec<_>>().join(" "))
            .eq_ignore_ascii_case(header_name)
    })
}

fn extract_heading_text(element: &ElementRef<'_>) -> String {
    let headline_selector = Selector::parse(".mw-headline").expect("valid headline selector");

    if let Some(headline) = element.select(&headline_selector).next() {
        return normalize_whitespace(&headline.text().collect::<Vec<_>>().join(" "));
    }

    normalize_whitespace(&element.text().collect::<Vec<_>>().join(" "))
}

fn extract_name(cell: &ElementRef<'_>) -> String {
    let link_selector = Selector::parse("a").expect("valid link selector");

    cell.select(&link_selector)
        .next()
        .map(|link| normalize_whitespace(&link.text().collect::<Vec<_>>().join(" ")))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| normalize_whitespace(&cell.text().collect::<Vec<_>>().join(" ")))
}

fn extract_image_url(cell: &ElementRef<'_>) -> Option<String> {
    let image_selector = Selector::parse("img").expect("valid image selector");
    let image = cell.select(&image_selector).next()?;

    let raw_src = image
        .value()
        .attr("src")
        .or_else(|| image.value().attr("data-src"))
        .or_else(|| {
            image
                .value()
                .attr("srcset")
                .and_then(|srcset| srcset.split(',').next())
                .and_then(|candidate| candidate.split_whitespace().next())
        })?;

    Some(resolve_wiki_url(raw_src))
}

fn extract_command(cell: &ElementRef<'_>) -> Vec<String> {
    let image_selector = Selector::parse("img").expect("valid image selector");
    let mut command = cell
        .select(&image_selector)
        .filter_map(|image| image.value().attr("alt"))
        .filter_map(direction_from_alt)
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if !command.is_empty() {
        return command;
    }

    let normalized = normalize_whitespace(&cell.text().collect::<Vec<_>>().join(" "))
        .replace('↑', " UP ")
        .replace('↓', " DOWN ")
        .replace('←', " LEFT ")
        .replace('→', " RIGHT ");

    for token in normalized.split_whitespace() {
        match token.to_ascii_uppercase().as_str() {
            "UP" => command.push("UP".to_string()),
            "DOWN" => command.push("DOWN".to_string()),
            "LEFT" => command.push("LEFT".to_string()),
            "RIGHT" => command.push("RIGHT".to_string()),
            _ => {}
        }
    }

    command
}

fn direction_from_alt(alt: &str) -> Option<&'static str> {
    let normalized = alt.to_ascii_lowercase();

    if normalized.contains("arrow up") {
        Some("UP")
    } else if normalized.contains("arrow down") {
        Some("DOWN")
    } else if normalized.contains("arrow left") {
        Some("LEFT")
    } else if normalized.contains("arrow right") {
        Some("RIGHT")
    } else {
        None
    }
}

fn resolve_wiki_url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else if path.starts_with("//") {
        format!("https:{}", path)
    } else if path.starts_with('/') {
        format!("{}{}", WIKI_ORIGIN, path)
    } else {
        format!("{}/{}", WIKI_ORIGIN, path.trim_start_matches("./"))
    }
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
