//! Offline Lucide + legacy Devicon catalog and bounded custom SVG validation.
use gpui::AssetSource;
use std::{borrow::Cow, collections::BTreeMap, io::Read as _, path::Path, sync::LazyLock};

pub const MAX_SVG_BYTES: usize = 128 * 1024;
pub const BUILTINS: &[(&str, &str)] = &[
    ("python", "Python"),
    ("rust", "Rust"),
    ("javascript", "JavaScript"),
    ("typescript", "TypeScript"),
    ("nodejs", "Node.js"),
    ("go", "Go"),
    ("java", "Java"),
    ("docker", "Docker"),
    ("bash", "Bash"),
    ("c", "C"),
    ("cplusplus", "C++"),
    ("git", "Git"),
];

pub fn builtin(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "python" => include_bytes!("../assets/tab-icons/python.svg"),
        "rust" => include_bytes!("../assets/tab-icons/rust.svg"),
        "javascript" => include_bytes!("../assets/tab-icons/javascript.svg"),
        "typescript" => include_bytes!("../assets/tab-icons/typescript.svg"),
        "nodejs" => include_bytes!("../assets/tab-icons/nodejs.svg"),
        "go" => include_bytes!("../assets/tab-icons/go.svg"),
        "java" => include_bytes!("../assets/tab-icons/java.svg"),
        "docker" => include_bytes!("../assets/tab-icons/docker.svg"),
        "bash" => include_bytes!("../assets/tab-icons/bash.svg"),
        "c" => include_bytes!("../assets/tab-icons/c.svg"),
        "cplusplus" => include_bytes!("../assets/tab-icons/cplusplus.svg"),
        "git" => include_bytes!("../assets/tab-icons/git.svg"),
        _ => return LUCIDE.get(name).map(|svg| svg.as_ref()),
    })
}

// AllAssets comes from the pinned gpui-kit-assets dependency. No network, directory scan,
// or generated platform-specific path is needed in the shipped application.
static LUCIDE: LazyLock<BTreeMap<String, Cow<'static, [u8]>>> = LazyLock::new(|| {
    let assets = gpui::assets::AllAssets;
    assets
        .list("icons/")
        .expect("embedded catalog")
        .into_iter()
        .filter_map(|path| {
            let name = path.strip_prefix("icons/")?.strip_suffix(".svg")?;
            let bytes = assets.load(&path).ok()??;
            Some((format!("lucide/{name}"), bytes))
        })
        .collect()
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    All,
    Brands,
    Development,
    Files,
    Arrows,
    Media,
    Nature,
    Life,
    Symbols,
}
impl Category {
    pub const ALL: [Self; 9] = [
        Self::All,
        Self::Brands,
        Self::Development,
        Self::Files,
        Self::Arrows,
        Self::Media,
        Self::Nature,
        Self::Life,
        Self::Symbols,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "全部图标",
            Self::Brands => "开发品牌",
            Self::Development => "开发与设备",
            Self::Files => "文件与办公",
            Self::Arrows => "箭头与方向",
            Self::Media => "影音与交流",
            Self::Nature => "自然与动物",
            Self::Life => "生活与出行",
            Self::Symbols => "符号与形状",
        }
    }
    pub fn icon(self) -> gpui::assets::IconName {
        use gpui::assets::IconName;
        match self {
            Self::All => IconName::Grid2x2,
            Self::Brands => IconName::Code,
            Self::Development => IconName::Terminal,
            Self::Files => IconName::Folder,
            Self::Arrows => IconName::ArrowRight,
            Self::Media => IconName::Music,
            Self::Nature => IconName::Leaf,
            Self::Life => IconName::Coffee,
            Self::Symbols => IconName::Shapes,
        }
    }
}

pub struct CatalogIcon {
    pub key: String,
    pub name: String,
    pub category: Category,
    keywords: String,
}

fn classify(name: &str) -> Category {
    let has = |words: &str| {
        words
            .split_whitespace()
            .any(|s| name.split('-').any(|part| part == s))
    };
    if has("arrow arrows chevron chevrons move corner navigation undo redo rotate flip fold unfold") { Category::Arrows }
    else if has("file files folder folders clipboard notebook book library archive calendar sheet table presentation pen pencil text type letter case heading list scan") { Category::Files }
    else if has("code terminal command braces brackets bug cpu gpu database server computer monitor laptop keyboard mouse ethernet network wifi usb bluetooth cable circuit binary git github gitlab hard drive memory router container") { Category::Development }
    else if has("audio video music camera film image images mic microphone headphone headphones volume speaker play pause podcast radio tv clapperboard disc drum piano guitar message mail send phone chat speech") { Category::Media }
    else if has("sun moon cloud tree trees leaf flower sprout plant mountain waves snowflake rain rainbow wind earth globe bird cat dog fish rabbit turtle squirrel snail bug feather paw cherry apple carrot banana grape citrus vegan wheat") { Category::Nature }
    else if has("car bus train plane ship boat bike foot footprints road route map compass home house building store shopping wallet credit bank dollar coin ticket gift cake coffee cup pizza sandwich beer wine utensils cooking bed bath sofa lamp heart hospital pill stethoscope school graduation briefcase hammer wrench scissors shirt hat watch glasses umbrella user users person baby smile angry laugh frown meh") { Category::Life }
    else { Category::Symbols }
}

pub static CATALOG: LazyLock<Vec<CatalogIcon>> = LazyLock::new(|| {
    BUILTINS
        .iter()
        .map(|(key, label)| CatalogIcon {
            key: (*key).into(),
            name: (*label).into(),
            category: Category::Brands,
            keywords: format!("{key} {label} 开发 品牌 编程").to_lowercase(),
        })
        .chain(LUCIDE.keys().map(|key| {
            let name = key.strip_prefix("lucide/").unwrap();
            let category = classify(name);
            let aliases = [
                ("heart", "爱心 喜欢 收藏"),
                ("star", "星星 收藏"),
                ("terminal", "终端 命令"),
                ("folder", "文件夹 目录"),
                ("file", "文件 文档"),
                ("search", "搜索 查找"),
                ("cat", "猫"),
                ("dog", "狗"),
                ("smile", "微笑 表情"),
                ("sun", "太阳 白天"),
                ("moon", "月亮 夜晚"),
                ("rocket", "火箭 启动"),
                ("settings", "设置 齿轮"),
                ("download", "下载"),
                ("upload", "上传"),
                ("cloud", "云"),
                ("database", "数据库"),
                ("code", "代码 编程"),
                ("lock", "锁 安全"),
                ("key", "钥匙"),
                ("trash", "删除 垃圾桶"),
                ("image", "图片 照片"),
                ("music", "音乐"),
                ("video", "视频"),
                ("camera", "相机"),
                ("bird", "鸟"),
                ("coffee", "咖啡"),
                ("gamepad", "游戏"),
                ("wifi", "无线 网络"),
            ]
            .into_iter()
            .filter(|(word, _)| name.split('-').any(|part| part == *word))
            .map(|(_, alias)| alias)
            .collect::<Vec<_>>()
            .join(" ");
            CatalogIcon {
                key: key.clone(),
                name: name.into(),
                category,
                keywords: format!(
                    "{} {} {}",
                    name.replace('-', " "),
                    category.label(),
                    aliases
                ),
            }
        }))
        .collect()
});

pub fn search(query: &str, category: Category) -> Vec<usize> {
    let query = query.trim().to_lowercase().replace('-', " ");
    let terms = query.split_whitespace().collect::<Vec<_>>();
    CATALOG
        .iter()
        .enumerate()
        .filter(|(_, icon)| {
            (category == Category::All || category == icon.category)
                && terms.iter().all(|term| icon.keywords.contains(term))
        })
        .map(|(index, _)| index)
        .collect()
}

pub fn validate_svg(bytes: &[u8]) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > MAX_SVG_BYTES {
        return Err("SVG 必须小于 128 KiB 且不为空。".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "SVG 不是 UTF-8 文本。")?;
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<svg")
        || [
            "<!",
            "<script",
            "<foreignobject",
            "<iframe",
            "<image",
            "<use",
            "<style",
            "@import",
            "href=",
        ]
        .iter()
        .any(|s| lower.contains(s))
    {
        return Err("SVG 含不支持的脚本、外链、嵌入资源或 XML 指令。".into());
    }
    for (ix, _) in lower.match_indices("url(") {
        if !lower[ix + 4..].trim_start().starts_with('#') {
            return Err("SVG 不允许加载外部资源。".into());
        }
    }
    let tree = resvg::usvg::Tree::from_str(text, &resvg::usvg::Options::default())
        .map_err(|_| "SVG 格式无效或不受支持。".to_string())?;
    if tree.size().width() > 1024. || tree.size().height() > 1024. {
        return Err("SVG 尺寸过大。".into());
    }
    Ok(text.to_owned())
}

pub fn read_local(path: &str) -> Result<String, String> {
    let file = Path::new(path);
    if !file.is_file()
        || !file
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        return Err("请选择已有的 .svg 文件。".into());
    }
    let metadata = std::fs::metadata(file).map_err(|e| format!("无法读取 SVG：{e}"))?;
    if metadata.len() > MAX_SVG_BYTES as u64 {
        return Err("SVG 文件超过 128 KiB。".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .map_err(|e| format!("无法读取 SVG：{e}"))?
        .take((MAX_SVG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取 SVG：{e}"))?;
    validate_svg(&bytes)
}

pub fn validate_url(raw: &str) -> Result<(), String> {
    let url = url::Url::parse(raw).map_err(|_| "SVG 地址无效。")?;
    let host = match url.host() {
        Some(url::Host::Domain(host)) => host.trim_end_matches('.'),
        _ => return Err("SVG 地址须为公网域名。".into()),
    };
    if !host.contains('.')
        || ["localhost", "local", "internal", "lan", "onion", "test"]
            .iter()
            .any(|suffix| host == *suffix || host.ends_with(&format!(".{suffix}")))
    {
        return Err("SVG 地址须为公网域名。".into());
    }
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
        || host.is_empty()
        || raw.len() > 2048
    {
        return Err("仅支持无凭据、无端口和查询参数的 HTTPS SVG 地址。".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_catalog_is_offline_searchable_unique_and_renderable() {
        assert!(CATALOG.len() >= 1800);
        let mut names = std::collections::HashSet::new();
        for icon in CATALOG.iter() {
            assert!(names.insert(&icon.key), "duplicate: {}", icon.key);
            let bytes = builtin(&icon.key).unwrap();
            assert!(
                resvg::usvg::Tree::from_data(bytes, &resvg::usvg::Options::default()).is_ok(),
                "{}",
                icon.key
            );
        }
        for category in Category::ALL {
            assert!(!search("", category).is_empty(), "{category:?}");
        }
        for query in [
            "rocket",
            "火箭",
            "file code",
            "文件夹",
            "PYTHON",
            "arrow-right",
        ] {
            assert!(!search(query, Category::All).is_empty(), "{query}");
        }
        assert!(search("no-such-icon-xyz", Category::All).is_empty());
        assert_eq!(search("", Category::Brands).len(), 12);
        assert!(search("python", Category::Nature).is_empty());
        assert!(builtin("lucide/not-an-icon").is_none());
    }

    #[test]
    fn bundled_logos_parse_and_custom_sources_are_bounded() {
        for (name, _) in BUILTINS {
            assert!(validate_svg(builtin(name).unwrap()).is_ok(), "{name}");
        }
        assert!(validate_svg(b"<svg><script>alert(1)</script></svg>").is_err());
        assert!(validate_svg(b"<!DOCTYPE svg><svg/>").is_err());
        assert!(validate_svg(b"<svg><image href='https://example.com/a'/></svg>").is_err());
        assert!(validate_svg(&vec![b'x'; MAX_SVG_BYTES + 1]).is_err());
        assert!(validate_url("https://raw.githubusercontent.com/example/icon.svg").is_ok());
        for url in [
            "file:///tmp/icon.svg",
            "http://example.com/icon.svg",
            "https://localhost/icon.svg",
            "https://127.0.0.1/icon.svg",
            "https://[::1]/icon.svg",
            "https://host.internal/icon.svg",
            "https://localhost./icon.svg",
            "https://example.com:8443/icon.svg",
            "https://example.com/icon.svg?token=secret",
            "https://user:pass@example.com/a.svg",
        ] {
            assert!(validate_url(url).is_err(), "{url}");
        }
    }
}
