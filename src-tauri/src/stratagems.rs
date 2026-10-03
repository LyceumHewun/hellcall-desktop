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
const CURRENT_STRATAGEMS_SECTION: &str = "Current Stratagems";
const MISSION_STRATAGEMS_SECTION: &str = "Mission Stratagems";

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StratagemLanguage {
    En,
    Zh,
}

impl StratagemLanguage {
    fn page_url(self) -> &'static str {
        match self {
            Self::En => WIKI_PAGE_URL,
            Self::Zh => "https://helldivers.wiki.gg/zh/wiki/%E6%88%98%E7%95%A5%E9%85%8D%E5%A4%87",
        }
    }

    fn api_url(self) -> &'static str {
        match self {
            Self::En => WIKI_API_URL,
            Self::Zh => "https://helldivers.wiki.gg/zh/api.php?action=parse&page=战略配备&prop=text&format=json",
        }
    }

    fn sections(self) -> (&'static str, &'static str) {
        match self {
            Self::En => (CURRENT_STRATAGEMS_SECTION, MISSION_STRATAGEMS_SECTION),
            Self::Zh => ("当前战略配备", "任务所需战略配备"),
        }
    }

    fn column_headers(self) -> [&'static str; 3] {
        match self {
            Self::En => ["Icon", "Name", "Stratagem Code"],
            Self::Zh => ["图标", "名称", "战略配备代码"],
        }
    }
}

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

pub async fn refresh_catalog(
    app_handle: &AppHandle,
    language: StratagemLanguage,
) -> Result<StratagemCatalog, String> {
    let client = reqwest::Client::builder()
        .user_agent("Hellcall Desktop Stratagem Updater/1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let html = fetch_stratagem_page_html(&client, language).await?;
    let items = parse_stratagems_from_html(&html, language)?;

    if !has_complete_stratagem_sections(&items, language) {
        return Err(
            "Parsed stratagem data was incomplete; keeping the existing cached catalog."
                .to_string(),
        );
    }

    let catalog = StratagemCatalog {
        updated_at_unix: Some(current_unix_timestamp()),
        source_url: language.page_url().to_string(),
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
    let temporary_path = path.with_extension("toml.tmp");
    fs::write(&temporary_path, content).map_err(|e| e.to_string())?;
    fs::rename(&temporary_path, path).map_err(|e| e.to_string())
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

async fn fetch_stratagem_page_html(
    client: &reqwest::Client,
    language: StratagemLanguage,
) -> Result<String, String> {
    match fetch_stratagem_page_html_from_api(client, language).await {
        Ok(html) => return Ok(html),
        Err(error) => {
            log::warn!(
                "Failed to fetch usable stratagem HTML from wiki API, falling back to page: {}",
                error
            );
        }
    }

    fetch_stratagem_page_html_from_page(client, language).await
}

async fn fetch_stratagem_page_html_from_api(
    client: &reqwest::Client,
    language: StratagemLanguage,
) -> Result<String, String> {
    let payload = client
        .get(language.api_url())
        .send()
        .await
        .map_err(|e| e.to_string())?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let api_response: WikiApiResponse =
        serde_json::from_str(&payload).map_err(|e| format!("Invalid wiki API response: {}", e))?;
    let html = api_response.parse.text.html;

    if !is_valid_wiki_html(&html) {
        return Err("Wiki API response did not include expected stratagem markup.".to_string());
    }

    Ok(html)
}

async fn fetch_stratagem_page_html_from_page(
    client: &reqwest::Client,
    language: StratagemLanguage,
) -> Result<String, String> {
    match client.get(language.page_url()).send().await {
        Ok(response) if response.status().is_success() => {
            let html = response.text().await.map_err(|e| e.to_string())?;
            if is_valid_wiki_html(&html) {
                return Ok(html);
            }

            Err("Wiki page response did not include expected stratagem markup.".to_string())
        }
        Ok(response) => Err(format!("Wiki page returned status {}.", response.status())),
        Err(error) => Err(format!("Failed to fetch wiki page directly: {}", error)),
    }
}

fn looks_like_cloudflare_challenge(html: &str) -> bool {
    html.contains("__cf_chl")
        || html.contains("Just a moment...")
        || html.contains("challenge-platform")
        || html.contains("Enable JavaScript and cookies to continue")
}

fn is_valid_wiki_html(html: &str) -> bool {
    !looks_like_cloudflare_challenge(html) && html.contains("mw-parser-output")
}

fn parse_stratagems_from_html(
    html: &str,
    language: StratagemLanguage,
) -> Result<Vec<Stratagem>, String> {
    let document = Html::parse_document(html);
    let container_selector =
        Selector::parse(".mw-parser-output").map_err(|e| format!("Invalid selector: {}", e))?;
    let details_selector =
        Selector::parse("details").map_err(|e| format!("Invalid selector: {}", e))?;
    let summary_selector =
        Selector::parse("summary").map_err(|e| format!("Invalid selector: {}", e))?;
    let table_selector =
        Selector::parse("table.wikitable").map_err(|e| format!("Invalid selector: {}", e))?;
    let content_root = document
        .select(&container_selector)
        .next()
        .ok_or_else(|| "Wiki response did not include .mw-parser-output".to_string())?;

    let mut items = Vec::new();

    for details in content_root.select(&details_selector) {
        let Some(summary) = details.select(&summary_selector).next() else {
            continue;
        };
        let summary_text = extract_text(&summary);
        let Some(section) = classify_stratagem_summary(&summary_text, language) else {
            continue;
        };
        let category = if section == language.sections().1 {
            section.to_string()
        } else {
            summary_text
        };

        for table in details.select(&table_selector) {
            items.extend(parse_stratagem_table(&table, section, &category, language));
        }
    }

    Ok(items)
}

fn parse_stratagem_table(
    table: &ElementRef<'_>,
    section: &str,
    category: &str,
    language: StratagemLanguage,
) -> Vec<Stratagem> {
    let row_selector = Selector::parse("tr").expect("valid row selector");
    let rows = table.select(&row_selector).collect::<Vec<_>>();
    if rows.is_empty() {
        return Vec::new();
    }

    let header_cells = direct_cells(&rows[0]);
    let [icon_header, name_header, command_header] = language.column_headers();
    let (Some(icon_index), Some(name_index), Some(command_index)) = (
        find_column_index(&header_cells, icon_header),
        find_column_index(&header_cells, name_header),
        find_column_index(&header_cells, command_header),
    ) else {
        return Vec::new();
    };
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
                category: category.to_string(),
                name,
                icon_url: extract_image_url(&cells[icon_index]).unwrap_or_default(),
                command,
            })
        })
        .collect()
}

fn classify_stratagem_summary(summary: &str, language: StratagemLanguage) -> Option<&'static str> {
    let normalized_summary = normalize_whitespace(summary);
    if normalized_summary.is_empty() {
        return None;
    }

    let (current_section, mission_section) = language.sections();
    if normalized_summary.eq_ignore_ascii_case(mission_section) {
        Some(mission_section)
    } else {
        Some(current_section)
    }
}

fn has_complete_stratagem_sections(items: &[Stratagem], language: StratagemLanguage) -> bool {
    let (current_section, mission_section) = language.sections();
    !items.is_empty()
        && items.iter().any(|item| item.section == current_section)
        && items.iter().any(|item| item.section == mission_section)
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

fn extract_text(element: &ElementRef<'_>) -> String {
    normalize_whitespace(&element.text().collect::<Vec<_>>().join(" "))
}

fn extract_name(cell: &ElementRef<'_>) -> String {
    let link_selector = Selector::parse("a").expect("valid link selector");

    cell.select(&link_selector)
        .next()
        .map(|link| extract_text(&link))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| extract_text(cell))
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

    let normalized = extract_text(cell)
        .replace("↑", " UP ")
        .replace("↓", " DOWN ")
        .replace("←", " LEFT ")
        .replace("→", " RIGHT ");

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

#[cfg(test)]
mod tests {
    use super::{
        compute_stratagem_id, has_complete_stratagem_sections, load_catalog_from_path,
        parse_stratagems_from_html, save_catalog_to_path, Stratagem, StratagemCatalog,
        StratagemLanguage, CURRENT_STRATAGEMS_SECTION, MISSION_STRATAGEMS_SECTION,
    };

    const DETAILS_FIXTURE: &str = r#"
<div class="mw-parser-output">
  <details>
    <summary>Orbital Strikes</summary>
    <table class="wikitable sortable">
      <tbody>
        <tr>
          <th>Icon</th>
          <th>Name</th>
          <th>Stratagem Code</th>
          <th>Base Cooldown</th>
          <th>Source</th>
        </tr>
        <tr>
          <td><img src="/images/orbital.png" /></td>
          <td><a href="/wiki/Orbital_Precision_Strike">Orbital Precision Strike</a></td>
          <td>
            <img alt="Stratagem Arrow Right.svg" src="/images/right.svg" />
            <img alt="Stratagem Arrow Right.svg" src="/images/right.svg" />
            <img alt="Stratagem Arrow Up.svg" src="/images/up.svg" />
          </td>
          <td>90s</td>
          <td>Bridge</td>
        </tr>
      </tbody>
    </table>
  </details>
  <details>
    <summary>Mission Stratagems</summary>
    <table class="wikitable sortable">
      <tbody>
        <tr>
          <th>Icon</th>
          <th>Name</th>
          <th>Stratagem Code</th>
        </tr>
        <tr>
          <td><img src="/images/reinforce.png" /></td>
          <td><a href="/wiki/Reinforce">Reinforce</a></td>
          <td>UP DOWN RIGHT LEFT UP</td>
        </tr>
      </tbody>
    </table>
  </details>
</div>
"#;

    #[test]
    fn parse_stratagems_uses_details_summary_grouping() {
        let items = parse_stratagems_from_html(DETAILS_FIXTURE, StratagemLanguage::En)
            .expect("fixture should parse");

        assert_eq!(items.len(), 2);
        assert_eq!(items[0].section, CURRENT_STRATAGEMS_SECTION);
        assert_eq!(items[0].category, "Orbital Strikes");
        assert_eq!(items[1].section, MISSION_STRATAGEMS_SECTION);
        assert_eq!(items[1].category, MISSION_STRATAGEMS_SECTION);
    }

    #[test]
    fn parse_stratagems_extracts_direction_from_image_alt_and_text_fallback() {
        let items = parse_stratagems_from_html(DETAILS_FIXTURE, StratagemLanguage::En)
            .expect("fixture should parse");

        assert_eq!(items[0].command, vec!["RIGHT", "RIGHT", "UP"]);
        assert_eq!(items[1].command, vec!["UP", "DOWN", "RIGHT", "LEFT", "UP"]);
    }

    #[test]
    fn parse_stratagems_preserves_name_and_icon_with_extra_columns() {
        let items = parse_stratagems_from_html(DETAILS_FIXTURE, StratagemLanguage::En)
            .expect("fixture should parse");

        assert_eq!(items[0].name, "Orbital Precision Strike");
        assert_eq!(
            items[0].icon_url,
            "https://helldivers.wiki.gg/images/orbital.png"
        );
    }

    #[test]
    fn completeness_check_requires_current_and_mission_sections() {
        let current_only = vec![Stratagem {
            id: String::new(),
            section: CURRENT_STRATAGEMS_SECTION.to_string(),
            category: "Orbital Strikes".to_string(),
            name: "Orbital Precision Strike".to_string(),
            icon_url: String::new(),
            command: vec!["RIGHT".to_string()],
        }];
        let mission_only = vec![Stratagem {
            id: String::new(),
            section: MISSION_STRATAGEMS_SECTION.to_string(),
            category: MISSION_STRATAGEMS_SECTION.to_string(),
            name: "Reinforce".to_string(),
            icon_url: String::new(),
            command: vec!["UP".to_string()],
        }];

        assert!(!has_complete_stratagem_sections(
            &current_only,
            StratagemLanguage::En
        ));
        assert!(!has_complete_stratagem_sections(
            &mission_only,
            StratagemLanguage::En
        ));
        assert!(has_complete_stratagem_sections(
            &parse_stratagems_from_html(DETAILS_FIXTURE, StratagemLanguage::En)
                .expect("fixture should parse"),
            StratagemLanguage::En,
        ));
    }

    // Representative markup from the Chinese wiki, including English names in mission tables.
    const CHINESE_FIXTURE: &str = r#"
<div class="mw-parser-output">
  <details>
    <summary>轨道攻击</summary>
    <table class="wikitable sortable"><tbody>
      <tr><th>图标</th><th>名称</th><th>战略配备代码</th><th>基础冷却</th><th>来源</th></tr>
      <tr>
        <td><a class="image"><img src="https://helldivers.wiki.gg/images/orbital.svg?f73d52" /></a></td>
        <td><a href="/zh/wiki/轨道加特林火力网">轨道加特林火力网</a></td>
        <td>
          <span><img alt="Stratagem Arrow Right.svg" /></span>
          <span><img alt="Stratagem Arrow Down.svg" /></span>
          <span><img alt="Stratagem Arrow Left.svg" /></span>
          <span><img alt="Stratagem Arrow Up.svg" /></span>
          <span><img alt="Stratagem Arrow Up.svg" /></span>
        </td>
        <td>70s</td><td>轨道加农炮</td>
      </tr>
    </tbody></table>
  </details>
  <details>
    <summary>任务所需战略配备</summary>
    <p>舰船</p>
    <table class="wikitable sortable"><tbody>
      <tr><th>图标</th><th>名称</th><th>战略配备代码</th><th>基础冷却</th></tr>
      <tr><td><img src="/images/reinforce.svg" /></td><td><a>Reinforce</a></td><td>↑ ↓ → ← ↑</td><td>0s</td></tr>
    </tbody></table>
    <p>任务</p>
    <table class="wikitable sortable"><tbody>
      <tr><th>名称</th><th>战略配备代码</th><th>图标</th></tr>
      <tr><td><a>地狱炸弹</a></td><td>↓ ↑ ← ↓ ↑ → ↓ ↑</td><td><img src="/images/hellbomb.svg" /></td></tr>
    </tbody></table>
    <table class="wikitable"><tr><th>名称</th></tr><tr><td>Ignore this unrelated table</td></tr></table>
  </details>
</div>
"#;

    #[test]
    fn chinese_wiki_parses_localized_headers_categories_and_all_mission_tables() {
        let items = parse_stratagems_from_html(CHINESE_FIXTURE, StratagemLanguage::Zh).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].name, "轨道加特林火力网");
        assert_eq!(items[0].section, "当前战略配备");
        assert_eq!(items[0].category, "轨道攻击");
        assert_eq!(
            items[0].icon_url,
            "https://helldivers.wiki.gg/images/orbital.svg?f73d52"
        );
        assert_eq!(items[0].command, ["RIGHT", "DOWN", "LEFT", "UP", "UP"]);
        assert_eq!(items[1].name, "Reinforce");
        assert_eq!(items[1].section, "任务所需战略配备");
        assert_eq!(items[1].category, "任务所需战略配备");
        assert_eq!(items[1].command, ["UP", "DOWN", "RIGHT", "LEFT", "UP"]);
        assert_eq!(items[2].name, "地狱炸弹");
        assert_eq!(
            items[2].icon_url,
            "https://helldivers.wiki.gg/images/hellbomb.svg"
        );
        assert!(has_complete_stratagem_sections(
            &items,
            StratagemLanguage::Zh
        ));
        assert!(!has_complete_stratagem_sections(
            &items[..1],
            StratagemLanguage::Zh
        ));
        assert!(!has_complete_stratagem_sections(
            &items[1..],
            StratagemLanguage::Zh
        ));
        assert!(!has_complete_stratagem_sections(
            &items,
            StratagemLanguage::En
        ));
    }

    #[test]
    fn wiki_language_is_explicit_and_matches_the_selected_source() {
        assert!(matches!(
            serde_json::from_str::<StratagemLanguage>("\"en\"").unwrap(),
            StratagemLanguage::En
        ));
        assert!(matches!(
            serde_json::from_str::<StratagemLanguage>("\"zh\"").unwrap(),
            StratagemLanguage::Zh
        ));
        assert!(serde_json::from_str::<StratagemLanguage>("\"fr\"").is_err());
        assert_eq!(StratagemLanguage::En.page_url(), super::WIKI_PAGE_URL);
        assert_eq!(StratagemLanguage::En.api_url(), super::WIKI_API_URL);
        assert!(StratagemLanguage::Zh.page_url().contains("/zh/wiki/"));
        assert_eq!(StratagemLanguage::Zh.api_url(), "https://helldivers.wiki.gg/zh/api.php?action=parse&page=战略配备&prop=text&format=json");
        assert!(
            parse_stratagems_from_html(CHINESE_FIXTURE, StratagemLanguage::En)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn changing_language_replaces_the_same_cache_and_keeps_command_ids() {
        let path = std::env::temp_dir().join(format!(
            "hellcall-stratagem-language-{}.toml",
            std::process::id()
        ));
        let english_items =
            parse_stratagems_from_html(DETAILS_FIXTURE, StratagemLanguage::En).unwrap();
        let english_mission_id = compute_stratagem_id(&english_items[1].command);
        let mut catalog = StratagemCatalog {
            updated_at_unix: Some(123),
            source_url: StratagemLanguage::En.page_url().to_string(),
            items: english_items,
        };
        save_catalog_to_path(&path, &catalog).unwrap();
        catalog.source_url = StratagemLanguage::Zh.page_url().to_string();
        catalog.items = parse_stratagems_from_html(CHINESE_FIXTURE, StratagemLanguage::Zh).unwrap();
        for item in &mut catalog.items {
            item.id = compute_stratagem_id(&item.command);
        }
        save_catalog_to_path(&path, &catalog).unwrap();
        let loaded = load_catalog_from_path(&path).unwrap();
        assert_eq!(loaded.source_url, StratagemLanguage::Zh.page_url());
        assert_eq!(loaded.items.len(), 3);
        assert_eq!(loaded.items[0].name, "轨道加特林火力网");
        assert_eq!(loaded.items[1].id, english_mission_id);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_cache_write_preserves_the_saved_catalog() {
        let path = std::env::temp_dir().join(format!(
            "hellcall-stratagem-write-failure-{}.toml",
            std::process::id()
        ));
        save_catalog_to_path(&path, &StratagemCatalog::default()).unwrap();
        let original = std::fs::read(&path).unwrap();
        let temporary_path = path.with_extension("toml.tmp");
        std::fs::create_dir(&temporary_path).unwrap();
        assert!(save_catalog_to_path(
            &path,
            &StratagemCatalog {
                source_url: StratagemLanguage::Zh.page_url().to_string(),
                ..StratagemCatalog::default()
            }
        )
        .is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_dir(temporary_path).unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
