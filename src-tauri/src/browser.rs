use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::accessibility::{
    AxNode, AxPropertyName, GetFullAxTreeParams,
};
use chromiumoxide::cdp::browser_protocol::dom::{
    BackendNodeId, GetBoxModelParams, ResolveNodeParams,
};
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, DispatchMouseEventParams,
    DispatchMouseEventType, MouseButton,
};
use chromiumoxide::cdp::browser_protocol::page::{NavigateParams, ReloadParams};
use chromiumoxide::handler::Handler;
use chromiumoxide::Page;
use futures::StreamExt;
use tokio::sync::Mutex;

// paths to check for DevToolsActivePort (for connecting to existing chrome)
const CHROME_PROFILES: &[&str] = &[
    "Library/Application Support/Google/Chrome",
    "Library/Application Support/Google/Chrome Canary",
    "Library/Application Support/Arc/User Data",
    "Library/Application Support/Chromium",
];

pub struct BrowserClient {
    browser: Browser,
    _handler_task: tokio::task::JoinHandle<()>,
    pages: Vec<Page>,
    selected_page_idx: usize,
    // snapshot state
    snapshot_id: u64,
    uid_to_backend_node: HashMap<String, BackendNodeId>,
}

impl BrowserClient {
    pub async fn connect() -> Result<Self> {
        // try to connect to existing chrome first
        if let Some(ws_url) = try_find_existing_chrome().await {
            println!("[browser] Connecting to existing Chrome at {}", ws_url);
            match Browser::connect(&ws_url).await {
                Ok((browser, handler)) => {
                    let handler_task = tokio::spawn(async move {
                        handler_loop(handler).await;
                    });

                    let pages = browser.pages().await.unwrap_or_default();
                    return Ok(Self {
                        browser,
                        _handler_task: handler_task,
                        pages,
                        selected_page_idx: 0,
                        snapshot_id: 0,
                        uid_to_backend_node: HashMap::new(),
                    });
                }
                Err(e) => {
                    println!("[browser] Failed to connect to existing Chrome: {}", e);
                }
            }
        }

        // no existing chrome, launch a new one using chromiumoxide
        println!("[browser] Launching Chrome with user profile...");
        let (browser, handler) = launch_chrome_with_profile().await?;

        let handler_task = tokio::spawn(async move {
            handler_loop(handler).await;
        });

        let pages = browser.pages().await.unwrap_or_default();
        Ok(Self {
            browser,
            _handler_task: handler_task,
            pages,
            selected_page_idx: 0,
            snapshot_id: 0,
            uid_to_backend_node: HashMap::new(),
        })
    }

    fn selected_page(&self) -> Result<&Page> {
        self.pages
            .get(self.selected_page_idx)
            .ok_or_else(|| anyhow!("no page selected"))
    }

    // refresh page list from browser
    async fn refresh_pages(&mut self) -> Result<()> {
        self.pages = self.browser.pages().await?;
        if self.selected_page_idx >= self.pages.len() && !self.pages.is_empty() {
            self.selected_page_idx = 0;
        }
        Ok(())
    }

    // tool: take_snapshot
    pub async fn take_snapshot(&mut self, verbose: bool) -> Result<String> {
        let page = self.selected_page()?;

        let resp = page
            .execute(GetFullAxTreeParams::builder().build())
            .await
            .context("failed to get a11y tree")?;

        self.snapshot_id += 1;
        self.uid_to_backend_node.clear();

        let nodes = resp.result.nodes;
        let snapshot_text = format_ax_tree(&nodes, self.snapshot_id, verbose, &mut self.uid_to_backend_node);

        Ok(snapshot_text)
    }

    // tool: click
    pub async fn click(&mut self, uid: &str, dbl_click: bool) -> Result<String> {
        let (x, y) = self.resolve_uid_to_point(uid).await?;
        let page = self.selected_page()?;

        // move mouse
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseMoved)
                .x(x)
                .y(y)
                .build()
                .unwrap(),
        )
        .await?;

        let click_count = if dbl_click { 2 } else { 1 };

        // mouse down
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MousePressed)
                .x(x)
                .y(y)
                .button(MouseButton::Left)
                .click_count(click_count)
                .build()
                .unwrap(),
        )
        .await?;

        // mouse up
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseReleased)
                .x(x)
                .y(y)
                .button(MouseButton::Left)
                .click_count(click_count)
                .build()
                .unwrap(),
        )
        .await?;

        let action = if dbl_click { "double clicked" } else { "clicked" };
        Ok(format!("Successfully {action} on element"))
    }

    // tool: hover
    pub async fn hover(&mut self, uid: &str) -> Result<String> {
        let (x, y) = self.resolve_uid_to_point(uid).await?;
        let page = self.selected_page()?;

        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseMoved)
                .x(x)
                .y(y)
                .build()
                .unwrap(),
        )
        .await?;

        Ok("Successfully hovered over element".to_string())
    }

    // tool: fill
    pub async fn fill(&mut self, uid: &str, value: &str) -> Result<String> {
        // click first to focus
        self.click(uid, false).await?;

        let page = self.selected_page()?;

        // clear existing content with ctrl+a then delete
        page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyDown)
                .key("a")
                .modifiers(2) // ctrl/cmd
                .build()
                .unwrap(),
        )
        .await?;
        page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyUp)
                .key("a")
                .build()
                .unwrap(),
        )
        .await?;

        // type each character
        for c in value.chars() {
            page.execute(
                DispatchKeyEventParams::builder()
                    .r#type(DispatchKeyEventType::Char)
                    .text(c.to_string())
                    .build()
                    .unwrap(),
            )
            .await?;
        }

        Ok("Successfully filled element".to_string())
    }

    // tool: press_key
    pub async fn press_key(&mut self, key: &str) -> Result<String> {
        let page = self.selected_page()?;

        // parse modifiers from key string like "Control+A" or "Enter"
        let parts: Vec<&str> = key.split('+').collect();
        let (modifiers, key_name) = if parts.len() > 1 {
            let mods = &parts[..parts.len() - 1];
            let key = parts[parts.len() - 1];
            let mut mod_flags = 0;
            for m in mods {
                match m.to_lowercase().as_str() {
                    "control" | "ctrl" => mod_flags |= 2,
                    "alt" | "option" => mod_flags |= 1,
                    "shift" => mod_flags |= 8,
                    "meta" | "cmd" | "command" => mod_flags |= 4,
                    _ => {}
                }
            }
            (mod_flags, key)
        } else {
            (0, key)
        };

        // key down
        page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyDown)
                .key(key_name)
                .modifiers(modifiers)
                .build()
                .unwrap(),
        )
        .await?;

        // key up
        page.execute(
            DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyUp)
                .key(key_name)
                .modifiers(modifiers)
                .build()
                .unwrap(),
        )
        .await?;

        Ok(format!("Successfully pressed key: {key}"))
    }

    // tool: navigate_page
    pub async fn navigate_page(
        &mut self,
        nav_type: &str,
        url: Option<&str>,
        ignore_cache: bool,
    ) -> Result<String> {
        let page = self.selected_page()?;

        match nav_type {
            "url" => {
                let url = url.ok_or_else(|| anyhow!("url required for type=url"))?;
                page.execute(NavigateParams::builder().url(url).build().unwrap())
                    .await?;
                Ok(format!("Successfully navigated to {url}"))
            }
            "back" => {
                // use js history.back()
                page.evaluate("history.back()").await?;
                Ok("Successfully navigated back".to_string())
            }
            "forward" => {
                page.evaluate("history.forward()").await?;
                Ok("Successfully navigated forward".to_string())
            }
            "reload" => {
                page.execute(
                    ReloadParams::builder()
                        .ignore_cache(ignore_cache)
                        .build(),
                )
                .await?;
                Ok("Successfully reloaded page".to_string())
            }
            _ => Err(anyhow!("unknown navigation type: {nav_type}")),
        }
    }

    // tool: wait_for
    pub async fn wait_for(&mut self, text: &str, timeout_ms: u64) -> Result<String> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        loop {
            let snapshot = self.take_snapshot(false).await?;
            if snapshot.contains(text) {
                return Ok(format!("Element with text \"{text}\" found"));
            }

            if start.elapsed() > timeout {
                return Err(anyhow!("timeout waiting for text: {text}"));
            }

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    // tool: upload_file
    pub async fn upload_file(&mut self, uid: &str, file_path: &str) -> Result<String> {
        let backend_node_id = self.get_backend_node_id(uid)?;
        let page = self.selected_page()?;

        // resolve node to get remote object
        let resolve_resp = page
            .execute(
                ResolveNodeParams::builder()
                    .backend_node_id(backend_node_id)
                    .build(),
            )
            .await?;

        let object_id = resolve_resp
            .result
            .object
            .object_id
            .ok_or_else(|| anyhow!("could not resolve element"))?;

        // set file via js
        let js = format!(
            r#"
            (function(files) {{
                const input = this;
                const dt = new DataTransfer();
                for (const f of files) {{
                    dt.items.add(new File([''], f));
                }}
                input.files = dt.files;
                input.dispatchEvent(new Event('change', {{ bubbles: true }}));
            }})(["{file_path}"])
            "#
        );

        page.evaluate(format!(
            "((obj) => {{ const el = obj; {js} }})(document.querySelector('[data-object-id=\"{}\"]'))",
            object_id.inner()
        ))
        .await?;

        Ok(format!("File uploaded: {file_path}"))
    }

    // tool: new_page
    pub async fn new_page(&mut self, url: &str) -> Result<String> {
        let page = self.browser.new_page(url).await?;
        self.pages.push(page);
        self.selected_page_idx = self.pages.len() - 1;
        Ok(format!("Created new page and navigated to {url}"))
    }

    // tool: list_pages
    pub async fn list_pages(&mut self) -> Result<String> {
        self.refresh_pages().await?;

        let mut result = String::new();
        for (idx, page) in self.pages.iter().enumerate() {
            let url = page.url().await?.unwrap_or_default();
            let selected = if idx == self.selected_page_idx {
                " [selected]"
            } else {
                ""
            };
            result.push_str(&format!("{idx}: {url}{selected}\n"));
        }

        if result.is_empty() {
            result = "No pages open".to_string();
        }

        Ok(result)
    }

    // tool: select_page
    pub async fn select_page(&mut self, page_idx: usize, bring_to_front: bool) -> Result<String> {
        self.refresh_pages().await?;

        if page_idx >= self.pages.len() {
            return Err(anyhow!(
                "page index {page_idx} out of range (0..{})",
                self.pages.len()
            ));
        }

        self.selected_page_idx = page_idx;

        if bring_to_front {
            let page = &self.pages[page_idx];
            page.bring_to_front().await?;
        }

        Ok(format!("Selected page {page_idx}"))
    }

    // helper: get backend node id from uid
    fn get_backend_node_id(&self, uid: &str) -> Result<BackendNodeId> {
        // validate snapshot id
        let parts: Vec<&str> = uid.split('_').collect();
        if parts.len() != 2 {
            return Err(anyhow!("invalid uid format: {uid}"));
        }

        let snapshot_id: u64 = parts[0]
            .parse()
            .map_err(|_| anyhow!("invalid snapshot id in uid"))?;

        if snapshot_id != self.snapshot_id {
            return Err(anyhow!(
                "stale uid from snapshot {snapshot_id}, current is {}. take a new snapshot first.",
                self.snapshot_id
            ));
        }

        self.uid_to_backend_node
            .get(uid)
            .copied()
            .ok_or_else(|| anyhow!("uid not found: {uid}"))
    }

    // helper: resolve uid to center point
    async fn resolve_uid_to_point(&self, uid: &str) -> Result<(f64, f64)> {
        let backend_node_id = self.get_backend_node_id(uid)?;
        let page = self.selected_page()?;

        let box_resp = page
            .execute(
                GetBoxModelParams::builder()
                    .backend_node_id(backend_node_id)
                    .build(),
            )
            .await
            .context("failed to get box model for element")?;

        let model = box_resp.result.model;
        // content quad: 4 points (x1,y1,x2,y2,x3,y3,x4,y4)
        let quad = model.content.inner();
        let x = (quad[0] + quad[2] + quad[4] + quad[6]) / 4.0;
        let y = (quad[1] + quad[3] + quad[5] + quad[7]) / 4.0;

        Ok((x, y))
    }
}

// handler event loop
async fn handler_loop(mut handler: Handler) {
    while let Some(event) = handler.next().await {
        if event.is_err() {
            break;
        }
    }
}

// try to find existing chrome with debugging enabled
async fn try_find_existing_chrome() -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();

    // check DevToolsActivePort files in known profile locations
    for profile in CHROME_PROFILES {
        let port_file = PathBuf::from(&home).join(profile).join("Default/DevToolsActivePort");

        if let Ok(content) = tokio::fs::read_to_string(&port_file).await {
            let lines: Vec<&str> = content.lines().collect();
            if lines.len() >= 2 {
                let port = lines[0].trim();
                let path = lines[1].trim();
                let ws_url = format!("ws://127.0.0.1:{port}{path}");
                return Some(ws_url);
            }
        }
    }

    // fallback: try localhost:9222
    if reqwest::get("http://127.0.0.1:9222/json/version")
        .await
        .is_ok()
    {
        return Some("http://127.0.0.1:9222".to_string());
    }

    None
}

// launch chrome using chromiumoxide with dedicated debug profile
async fn launch_chrome_with_profile() -> Result<(Browser, Handler)> {
    let home = std::env::var("HOME").unwrap_or_default();

    // chrome requires a NON-DEFAULT user data dir for remote debugging
    // using the default chrome profile path doesn't work - chrome treats it specially
    // so we create a dedicated debug profile that's separate from the user's main profile
    let user_data_dir = PathBuf::from(&home).join(".taskhomie-chrome");

    println!("[browser] Using debug profile: {:?}", user_data_dir);

    // disable_default_args() skips puppeteer automation flags that break normal browser usage
    // (like --disable-extensions, --disable-sync, --enable-automation, etc.)
    let config = BrowserConfig::builder()
        .disable_default_args()
        .with_head()
        .user_data_dir(&user_data_dir)
        .viewport(None)
        .build()
        .map_err(|e| anyhow!("failed to build browser config: {}", e))?;

    Browser::launch(config)
        .await
        .context("failed to launch chrome")
}

// format a11y tree to text snapshot
fn format_ax_tree(
    nodes: &[AxNode],
    snapshot_id: u64,
    verbose: bool,
    uid_map: &mut HashMap<String, BackendNodeId>,
) -> String {
    // build parent->children map
    let mut children_map: HashMap<String, Vec<&AxNode>> = HashMap::new();
    let mut node_map: HashMap<String, &AxNode> = HashMap::new();
    let mut root_id: Option<String> = None;

    for node in nodes {
        let id = node.node_id.inner().to_string();
        node_map.insert(id.clone(), node);

        if let Some(ref parent_id) = node.parent_id {
            children_map
                .entry(parent_id.inner().to_string())
                .or_default()
                .push(node);
        } else {
            root_id = Some(id);
        }
    }

    let mut output = String::new();
    let mut node_index = 0u64;

    if let Some(root_id) = root_id {
        if let Some(root) = node_map.get(&root_id) {
            format_node(
                root,
                &children_map,
                &node_map,
                0,
                snapshot_id,
                &mut node_index,
                uid_map,
                verbose,
                &mut output,
            );
        }
    }

    output
}

fn format_node(
    node: &AxNode,
    children_map: &HashMap<String, Vec<&AxNode>>,
    node_map: &HashMap<String, &AxNode>,
    depth: usize,
    snapshot_id: u64,
    node_index: &mut u64,
    uid_map: &mut HashMap<String, BackendNodeId>,
    verbose: bool,
    output: &mut String,
) {
    // skip ignored nodes unless verbose
    if node.ignored && !verbose {
        // still process children
        if let Some(child_ids) = &node.child_ids {
            for child_id in child_ids {
                if let Some(child) = node_map.get(child_id.inner()) {
                    format_node(
                        child,
                        children_map,
                        node_map,
                        depth,
                        snapshot_id,
                        node_index,
                        uid_map,
                        verbose,
                        output,
                    );
                }
            }
        }
        return;
    }

    let uid = format!("{}_{}", snapshot_id, *node_index);
    *node_index += 1;

    // store backend node id mapping
    if let Some(backend_id) = node.backend_dom_node_id {
        uid_map.insert(uid.clone(), backend_id);
    }

    // build attributes
    let indent = "  ".repeat(depth);
    let mut attrs = vec![format!("uid={uid}")];

    // role
    if let Some(ref role) = node.role {
        if let Some(ref val) = role.value {
            if let Some(s) = val.as_str() {
                if s != "none" {
                    attrs.push(s.to_string());
                } else {
                    attrs.push("ignored".to_string());
                }
            }
        }
    }

    // name
    if let Some(ref name) = node.name {
        if let Some(ref val) = name.value {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    attrs.push(format!("\"{}\"", s.replace('"', "\\\"")));
                }
            }
        }
    }

    // properties
    if let Some(ref props) = node.properties {
        for prop in props {
            let name = &prop.name;
            if let Some(ref val) = prop.value.value {
                match name {
                    AxPropertyName::Focusable => {
                        if val.as_bool() == Some(true) {
                            attrs.push("focusable".to_string());
                        }
                    }
                    AxPropertyName::Focused => {
                        if val.as_bool() == Some(true) {
                            attrs.push("focused".to_string());
                        }
                    }
                    AxPropertyName::Disabled => {
                        if val.as_bool() == Some(true) {
                            attrs.push("disabled".to_string());
                        }
                    }
                    AxPropertyName::Expanded => {
                        if val.as_bool() == Some(true) {
                            attrs.push("expanded".to_string());
                        }
                    }
                    AxPropertyName::Selected => {
                        if val.as_bool() == Some(true) {
                            attrs.push("selected".to_string());
                        }
                    }
                    AxPropertyName::Checked => {
                        if let Some(s) = val.as_str() {
                            attrs.push(format!("checked={s}"));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    output.push_str(&format!("{}{}\n", indent, attrs.join(" ")));

    // recurse to children
    if let Some(child_ids) = &node.child_ids {
        for child_id in child_ids {
            if let Some(child) = node_map.get(child_id.inner()) {
                format_node(
                    child,
                    children_map,
                    node_map,
                    depth + 1,
                    snapshot_id,
                    node_index,
                    uid_map,
                    verbose,
                    output,
                );
            }
        }
    }
}

// thread-safe wrapper
pub type SharedBrowserClient = Arc<Mutex<Option<BrowserClient>>>;

pub fn create_shared_browser_client() -> SharedBrowserClient {
    Arc::new(Mutex::new(None))
}
