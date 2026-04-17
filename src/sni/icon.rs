use anyhow::Result;
use std::path::Path;

pub fn resolve_icon(icon_str: &str) -> Result<()> {
    if icon_str.is_empty() {
        return Ok(());
    }

    let path = Path::new(icon_str);
    if path.is_absolute() || icon_str.contains('/') || path.exists() {
        if let Some(ext) = path.extension() {
            match ext.to_string_lossy().to_lowercase().as_str() {
                "png" => {
                    let _ = image::open(icon_str)?;
                    return Ok(());
                }
                "svg" => {
                    return load_svg(icon_str);
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn load_svg(path: &str) -> Result<()> {
    let opt = usvg::Options::default();
    let data = std::fs::read(path)?;
    let _tree = usvg::Tree::from_data(&data, &opt)
        .map_err(|e| anyhow::anyhow!("failed to parse SVG: {}", e))?;
    Ok(())
}
