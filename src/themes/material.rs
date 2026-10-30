use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime},
};

use anyhow::{Context as _, bail};
use gpui::{App, Global, Hsla, Rgba};
use material_colors::{
    color::Argb,
    palette::TonalPalette,
    quantize::{Quantizer, QuantizerCelebi},
    score::Score,
    theme::ThemeBuilder,
};
use serde::Deserialize;

use super::{TerminalTheme, apply};
use crate::settings::SettingsStore;

pub const IDS: [&str; 2] = ["material-you-light", "material-you-dark"];

pub fn known(id: &str) -> bool {
    IDS.contains(&id)
}
const SCRIPT: &str = "print(JSON.stringify(desktops().map(d => { d.currentConfigGroup = ['Wallpaper', d.wallpaperPlugin, 'General']; return {screen: d.screen, image: d.readConfig('Image', '')}; })))";

pub struct MaterialYou(pub Vec<TerminalTheme>);
impl Global for MaterialYou {}

impl MaterialYou {
    pub fn theme(&self, id: &str) -> &TerminalTheme {
        self.0
            .iter()
            .find(|theme| theme.id == id)
            .expect("Material You id")
    }
}

#[derive(Deserialize)]
struct Desktop {
    screen: i32,
    image: String,
}

#[derive(Clone, Debug, PartialEq)]
struct Wallpaper {
    path: PathBuf,
    modified: SystemTime,
    length: u64,
}

pub fn start(cx: &mut App) -> anyhow::Result<()> {
    let settings = SettingsStore::get(cx);
    let worn = known(&settings.theme) || settings.dark_theme.as_deref().is_some_and(known);
    let initial = wallpaper().and_then(|wallpaper| {
        let theme = extract(&wallpaper)?;
        Ok((wallpaper, theme))
    });
    let shown = match initial {
        Ok((wallpaper, theme)) => {
            log::info!("material-you: follows {}", wallpaper.path.display());
            cx.set_global(theme);
            Some(wallpaper)
        }
        Err(error) if worn => return Err(error.context("settings wear Material You")),
        Err(error) => {
            log::warn!("material-you: unavailable: {error:#}");
            None
        }
    };
    watch(shown, cx);
    Ok(())
}

fn watch(mut shown: Option<Wallpaper>, cx: &mut App) {
    cx.spawn(async move |cx| {
        let mut failed = String::new();
        loop {
            cx.background_executor().timer(Duration::from_secs(3)).await;
            let previous = shown.clone();
            let next = cx
                .background_executor()
                .spawn(async move {
                    let wallpaper = wallpaper()?;
                    if previous.as_ref() == Some(&wallpaper) {
                        return Ok(None);
                    }
                    let theme = extract(&wallpaper)?;
                    Ok::<_, anyhow::Error>(Some((wallpaper, theme)))
                })
                .await;
            match next {
                Ok(Some((wallpaper, theme))) => {
                    shown = Some(wallpaper);
                    failed.clear();
                    cx.update(|cx| {
                        log::info!("material-you: wallpaper changed; colors regenerated");
                        cx.set_global(theme);
                        let settings = SettingsStore::get(cx);
                        if known(&settings.theme)
                            || settings.dark_theme.as_deref().is_some_and(known)
                        {
                            apply(cx);
                        }
                        cx.refresh_windows();
                    });
                }
                Ok(None) => failed.clear(),
                Err(error) => {
                    let error = format!("{error:#}");
                    if error != failed {
                        log::error!("material-you: {error}; existing colors stay");
                        failed = error;
                    }
                }
            }
        }
    })
    .detach();
}

fn wallpaper() -> anyhow::Result<Wallpaper> {
    let mut child = Command::new("qdbus6")
        .args([
            "org.kde.plasmashell",
            "/PlasmaShell",
            "org.kde.PlasmaShell.evaluateScript",
            SCRIPT,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("material-you: querying KDE wallpaper")?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(2) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("material-you: KDE wallpaper query timed out");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let output = child.wait_with_output()?;
    anyhow::ensure!(
        output.status.success(),
        "material-you: KDE wallpaper query: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    let mut desktops: Vec<Desktop> =
        serde_json::from_str(text.trim()).context("material-you: KDE wallpaper response")?;
    desktops.sort_by_key(|desktop| desktop.screen);
    let image = desktops
        .into_iter()
        .find(|desktop| desktop.screen >= 0 && !desktop.image.is_empty())
        .context("material-you: no image wallpaper on a KDE screen")?
        .image;
    wallpaper_file(&image)
}

fn wallpaper_file(image: &str) -> anyhow::Result<Wallpaper> {
    let mut path = if image.starts_with("file:") {
        url::Url::parse(image)?
            .to_file_path()
            .map_err(|()| anyhow::anyhow!("material-you: invalid wallpaper URL"))?
    } else {
        PathBuf::from(image)
    };
    if path.is_dir() {
        let folder = path.join("contents/images");
        let mut images = fs::read_dir(&folder)
            .with_context(|| format!("material-you: wallpaper package {}", folder.display()))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        images.retain(|path| path.is_file());
        images.sort();
        path = images
            .into_iter()
            .next()
            .context("material-you: empty wallpaper package")?;
    }
    let metadata = fs::metadata(&path)
        .with_context(|| format!("material-you: wallpaper {}", path.display()))?;
    Ok(Wallpaper {
        path,
        modified: metadata.modified()?,
        length: metadata.len(),
    })
}

fn extract(wallpaper: &Wallpaper) -> anyhow::Result<MaterialYou> {
    let image = image::ImageReader::open(&wallpaper.path)?
        .with_guessed_format()?
        .decode()
        .with_context(|| format!("material-you: decode {}", wallpaper.path.display()))?
        .thumbnail(128, 128)
        .to_rgba8();
    let pixels: Vec<_> = image
        .pixels()
        .filter(|p| p[3] == 255)
        .map(|p| Argb::new(255, p[0], p[1], p[2]))
        .collect();
    anyhow::ensure!(
        !pixels.is_empty(),
        "material-you: wallpaper has no opaque pixels"
    );
    let quantized = QuantizerCelebi::quantize(&pixels, 128);
    let colors = Score::score(&quantized.color_to_count, Some(1), None, None);
    let seed = *colors
        .first()
        .context("material-you: no wallpaper source color")?;
    log::info!(
        "material-you: source color {seed} from {}",
        wallpaper.path.display()
    );
    Ok(generate(seed))
}

fn generate(seed: Argb) -> MaterialYou {
    let theme = ThemeBuilder::with_source(seed).build();
    let pair = [(&theme.schemes.light, false), (&theme.schemes.dark, true)];
    let themes = pair
        .map(|(scheme, dark)| {
            let tone = if dark { 80 } else { 40 };
            let mut ansi = [color(scheme.on_surface); 16];
            for (index, hue) in [
                (1, 25.0),
                (2, 145.0),
                (3, 90.0),
                (4, 270.0),
                (5, 325.0),
                (6, 200.0),
            ] {
                let palette = TonalPalette::from_hue_and_chroma(hue, 40.0);
                ansi[index] = color(palette.tone(tone));
                ansi[index + 8] = color(palette.tone(if dark { 90 } else { 30 }));
            }
            ansi[0] = color(theme.palettes.neutral.tone(if dark { 15 } else { 25 }));
            ansi[8] = color(theme.palettes.neutral.tone(45));
            ansi[7] = color(scheme.on_surface_variant);
            ansi[15] = color(scheme.on_surface);
            TerminalTheme {
                id: IDS[usize::from(dark)].into(),
                name: if dark {
                    "Material You Dark (Wallpaper)"
                } else {
                    "Material You Light (Wallpaper)"
                }
                .into(),
                background: color(scheme.surface),
                foreground: color(scheme.on_surface),
                cursor: color(scheme.primary),
                cursor_text: color(scheme.on_primary),
                selection_background: color(scheme.primary_container),
                selection_foreground: Some(color(scheme.on_primary_container)),
                ansi,
                ely: false,
            }
        })
        .to_vec();
    MaterialYou(themes)
}

fn color(argb: Argb) -> Hsla {
    Rgba {
        r: f32::from(argb.red) / 255.0,
        g: f32::from(argb.green) / 255.0,
        b: f32::from(argb.blue) / 255.0,
        a: 1.0,
    }
    .into()
}
