#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod language;
mod orientation;
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

macro_rules! local_bail {
    ($en:literal,$sk:literal $(,$args:expr)* $(,)?)=>{
        if language::code()=="sk" {bail!($sk $(,$args)*);}else{bail!($en $(,$args)*);}
    };
}
macro_rules! local_println {
    ($en:literal,$sk:literal $(,$args:expr)* $(,)?)=>{
        if language::code()=="sk" {println!($sk $(,$args)*);}else{println!($en $(,$args)*);}
    };
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Photo {
    corners: [[f64; 2]; 4],
    rotation: u16,
}
#[derive(Serialize, Deserialize)]
struct Page {
    name: String,
    width: u32,
    height: u32,
    data: String,
    photos: Vec<Photo>,
}
#[derive(Deserialize)]
struct Plan {
    pages: Vec<PlanPage>,
}
#[derive(Deserialize)]
struct PlanPage {
    width: u32,
    height: u32,
    photos: Vec<Photo>,
}
#[derive(Clone, Copy, Debug)]
struct Rect {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

// White scanner gaps are detected on a reduced copy; original pixels are used for export.
fn detect(src: &RgbImage) -> Vec<Photo> {
    let small = image::imageops::resize(
        src,
        1200.min(src.width()),
        ((src.height() as f64 * (1200.min(src.width()) as f64 / src.width() as f64)).round()
            as u32)
            .max(1),
        FilterType::Triangle,
    );
    let (w, h) = small.dimensions();
    let dark: Vec<bool> = small
        .pixels()
        .enumerate()
        .map(|(i, p)| {
            let x = i as u32 % w;
            let y = i as u32 / w;
            x >= 2 && y >= 2 && x + 2 < w && y + 2 < h && p.0.iter().any(|&c| c < 235)
        })
        .collect();
    let mut rects = Vec::new();
    split(&dark, w, Rect { x: 0, y: 0, w, h }, &mut rects, 0);
    rects.retain(|r| {
        r.w > w / 12 && r.h > h / 12 && r.w as u64 * r.h as u64 > w as u64 * h as u64 / 100
    });
    rects.sort_by_key(|r| (r.y / (h / 25).max(1), r.x));
    let sx = src.width() as f64 / w as f64;
    let sy = src.height() as f64 / h as f64;
    rects
        .into_iter()
        .map(|r| {
            let x = (r.x as f64 * sx).max(0.0);
            let y = (r.y as f64 * sy).max(0.0);
            let right = (((r.x + r.w) as f64 * sx) - 1.0).min((src.width() - 1) as f64);
            let bottom = (((r.y + r.h) as f64 * sy) - 1.0).min((src.height() - 1) as f64);
            Photo {
                corners: [[x, y], [right, y], [right, bottom], [x, bottom]],
                rotation: 0,
            }
        })
        .collect()
}
fn split(mask: &[bool], stride: u32, r: Rect, out: &mut Vec<Rect>, depth: usize) {
    if depth > 12 || r.w < 10 || r.h < 10 {
        return;
    }
    let mut cols = vec![0u32; r.w as usize];
    let mut rows = vec![0u32; r.h as usize];
    for y in 0..r.h {
        for x in 0..r.w {
            if mask[((r.y + y) * stride + r.x + x) as usize] {
                cols[x as usize] += 1;
                rows[y as usize] += 1;
            }
        }
    }
    let Some(x0) = cols.iter().position(|&v| v > r.h / 100) else {
        return;
    };
    let x1 = cols.iter().rposition(|&v| v > r.h / 100).unwrap();
    let Some(y0) = rows.iter().position(|&v| v > r.w / 100) else {
        return;
    };
    let y1 = rows.iter().rposition(|&v| v > r.w / 100).unwrap();
    let tight = Rect {
        x: r.x + x0 as u32,
        y: r.y + y0 as u32,
        w: (x1 - x0 + 1) as u32,
        h: (y1 - y0 + 1) as u32,
    };
    if tight.w + 4 < r.w || tight.h + 4 < r.h {
        split(mask, stride, tight, out, depth + 1);
        return;
    }
    let gap = |v: &[u32], limit: u32| -> Option<(usize, usize)> {
        let mut best = None;
        let mut start = None;
        for (i, &n) in v.iter().enumerate() {
            if n <= limit {
                if start.is_none() {
                    start = Some(i);
                }
            } else if let Some(a) = start.take() {
                if a > v.len() / 12
                    && i < v.len() * 11 / 12
                    && i - a >= 2
                    && best.map_or(true, |(s, e)| i - a > e - s)
                {
                    best = Some((a, i));
                }
            }
        }
        if best.is_some() {
            return best;
        }
        // Slightly skewed prints need not share a completely white column.
        // Look for a narrow, deep valley, comparing both sides of the gutter.
        let reach = (v.len() / 50).max(8);
        let mut candidate = None;
        let mut score = 0.65f64;
        for i in v.len() / 12..v.len() * 11 / 12 {
            if i < reach || i + reach >= v.len() {
                continue;
            }
            let left = v[i - reach..i - reach / 2]
                .iter()
                .map(|&n| n as f64)
                .sum::<f64>()
                / (reach - reach / 2) as f64;
            let right = v[i + reach / 2..i + reach]
                .iter()
                .map(|&n| n as f64)
                .sum::<f64>()
                / (reach - reach / 2) as f64;
            let baseline = left.min(right);
            if baseline < 10.0 {
                continue;
            }
            let ratio = v[i] as f64 / baseline;
            if ratio < score {
                score = ratio;
                candidate = Some((i, i + 1));
            }
        }
        candidate
    };
    let gx = gap(&cols, (r.h / 100).max(1));
    let gy = gap(&rows, (r.w / 100).max(1));
    let vertical = match (gx, gy) {
        (Some((a, b)), Some((c, d))) => (b - a) as f64 / r.w as f64 >= (d - c) as f64 / r.h as f64,
        (Some(_), None) => true,
        _ => false,
    };
    if vertical {
        let (a, b) = gx.unwrap();
        split(mask, stride, Rect { w: a as u32, ..r }, out, depth + 1);
        split(
            mask,
            stride,
            Rect {
                x: r.x + b as u32,
                w: r.w - b as u32,
                ..r
            },
            out,
            depth + 1,
        );
    } else if let Some((a, b)) = gy {
        split(mask, stride, Rect { h: a as u32, ..r }, out, depth + 1);
        split(
            mask,
            stride,
            Rect {
                y: r.y + b as u32,
                h: r.h - b as u32,
                ..r
            },
            out,
            depth + 1,
        );
    } else {
        out.push(tight);
    }
}
fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}
fn crop(src: &RgbImage, p: &Photo) -> Result<RgbImage> {
    let c = p.corners;
    if ![0, 90, 180, 270].contains(&p.rotation) {
        local_bail!("Invalid rotation", "Neplatne otocenie");
    }
    for a in c {
        if !a[0].is_finite()
            || !a[1].is_finite()
            || a[0] < 0.0
            || a[1] < 0.0
            || a[0] > (src.width() - 1) as f64
            || a[1] > (src.height() - 1) as f64
        {
            local_bail!("Crop corner outside the image", "Roh mimo obrazka");
        }
    }
    let mut sign = 0.0f64;
    for i in 0..4 {
        let a = c[i];
        let b = c[(i + 1) % 4];
        let d = c[(i + 2) % 4];
        let cross = (b[0] - a[0]) * (d[1] - b[1]) - (b[1] - a[1]) * (d[0] - b[0]);
        if cross <= 0.01 || (sign != 0.0 && cross * sign <= 0.0) {
            local_bail!(
                "Crop corners must form a convex polygon, clockwise from top left",
                "Rohy musia tvorit konvexny vyrez v poradi zlava hore v smere hodin"
            );
        }
        sign = cross;
    }
    let w = ((dist(c[0], c[1]) + dist(c[3], c[2])) / 2.0)
        .round()
        .max(1.0) as u32
        + 1;
    let h = ((dist(c[0], c[3]) + dist(c[1], c[2])) / 2.0)
        .round()
        .max(1.0) as u32
        + 1;
    if w as u64 * h as u64 > 100_000_000 {
        local_bail!("The crop is too large", "Vyrez je prilis velky");
    }
    let dst = RgbImage::from_fn(w, h, |x, y| {
        let u = x as f64 / (w - 1) as f64;
        let v = y as f64 / (h - 1) as f64;
        let sx = ((1.0 - v) * ((1.0 - u) * c[0][0] + u * c[1][0])
            + v * ((1.0 - u) * c[3][0] + u * c[2][0]))
            .clamp(0.0, (src.width() - 1) as f64);
        let sy = ((1.0 - v) * ((1.0 - u) * c[0][1] + u * c[1][1])
            + v * ((1.0 - u) * c[3][1] + u * c[2][1]))
            .clamp(0.0, (src.height() - 1) as f64);
        let ix = sx.floor() as u32;
        let iy = sy.floor() as u32;
        let fx = sx - ix as f64;
        let fy = sy - iy as f64;
        let mut rgb = [0u8; 3];
        for k in 0..3 {
            rgb[k] = ((1.0 - fy)
                * ((1.0 - fx) * src.get_pixel(ix, iy)[k] as f64
                    + fx * src.get_pixel((ix + 1).min(src.width() - 1), iy)[k] as f64)
                + fy * ((1.0 - fx) * src.get_pixel(ix, (iy + 1).min(src.height() - 1))[k] as f64
                    + fx * src.get_pixel(
                        (ix + 1).min(src.width() - 1),
                        (iy + 1).min(src.height() - 1),
                    )[k] as f64))
                .round() as u8;
        }
        Rgb(rgb)
    });
    Ok(match p.rotation {
        90 => image::imageops::rotate90(&dst),
        180 => image::imageops::rotate180(&dst),
        270 => image::imageops::rotate270(&dst),
        _ => dst,
    })
}
fn poppler() -> PathBuf {
    if let Some(p) = env::var_os("PDFTOPPM") {
        return p.into();
    }
    let local = env::current_exe()
        .unwrap_or_default()
        .parent()
        .unwrap_or(Path::new("."))
        .join("poppler")
        .join(if cfg!(windows) {
            "pdftoppm.exe"
        } else {
            "pdftoppm"
        });
    if local.is_file() {
        local
    } else {
        PathBuf::from("pdftoppm")
    }
}
fn run() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let input = match args.next() {
        Some(p) => p,
        None => {
            local_bail!(
                "Usage: photo-splitter INPUT [--out DIRECTORY] [--plan plan.json] [--dpi 300]",
                "Pouzitie: photo-splitter VSTUP [--out PRIECINOK] [--plan plan.json] [--dpi 300]"
            )
        }
    };
    let input = PathBuf::from(input);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis();
    let mut out = input
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("rozdelene-fotky-{stamp}"));
    let mut plan = None;
    let mut dpi = 300u32;
    let mut no_open = false;
    let mut prepare_ui = false;
    while let Some(a) = args.next() {
        match a.to_str().unwrap_or("") {
            "--no-open" => no_open = true,
            "--prepare-ui" => {
                prepare_ui = true;
                no_open = true;
            }
            "--out" => {
                out = args
                    .next()
                    .context(language::text(
                        "Missing output directory",
                        "Chyba priecinok",
                    ))?
                    .into()
            }
            "--plan" => {
                plan = Some(PathBuf::from(
                    args.next()
                        .context(language::text("Missing plan file", "Chyba plan"))?,
                ))
            }
            "--dpi" => {
                dpi = args
                    .next()
                    .context(language::text("Missing DPI value", "Chyba DPI"))?
                    .to_str()
                    .context(language::text("Invalid DPI value", "Neplatne DPI"))?
                    .parse()?
            }
            _ => local_bail!("Unknown option: {:?}", "Neznamy parameter: {:?}", a),
        }
    }
    if !(72..=1200).contains(&dpi) {
        local_bail!("DPI must be between 72 and 1200", "DPI musi byt 72 az 1200");
    }
    if out.exists() && fs::read_dir(&out)?.next().is_some() {
        local_bail!(
            "The output directory already contains files. Choose a new --out directory.",
            "Vystupny priecinok uz obsahuje subory. Vyber novy --out."
        );
    }
    let temp = tempfile::tempdir()?;
    let mut orienter = orientation::Orienter::new(temp.path())?;
    let mut inputs = vec![];
    let is_pdf = input
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .eq_ignore_ascii_case("pdf");
    if is_pdf {
        let info = poppler().with_file_name(if cfg!(windows) {
            "pdfinfo.exe"
        } else {
            "pdfinfo"
        });
        let result = background_command(info)
            .arg(&input)
            .output()
            .context(language::text(
                "PDF information tool is unavailable",
                "Nedostupny PDF informacny nastroj",
            ))?;
        if !result.status.success() {
            local_bail!(
                "Could not open PDF: {}",
                "PDF sa nepodarilo otvorit: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        let count = String::from_utf8_lossy(&result.stdout)
            .lines()
            .find_map(|l| {
                l.strip_prefix("Pages:")
                    .and_then(|n| n.trim().parse::<usize>().ok())
            })
            .context(language::text(
                "Could not read the PDF page count",
                "Nedostupny pocet stran PDF",
            ))?;
        for i in 0..count {
            inputs.push(temp.path().join(format!("page-{:04}.png", i + 1)));
        }
    } else {
        inputs.push(input.clone());
    }
    if inputs.is_empty() {
        local_bail!("No pages found", "Nenasli sa ziadne strany");
    }
    let saved: Option<Plan> = plan
        .map(|p| -> Result<Plan> { Ok(serde_json::from_slice(&fs::read(p)?)?) })
        .transpose()?;
    if let Some(p) = &saved {
        if p.pages.len() != inputs.len() {
            local_bail!(
                "The page count does not match the plan",
                "Pocet stran nezodpoveda planu"
            );
        }
    }
    fs::create_dir_all(&out)?;
    let mut pages = vec![];
    let mut orientation_checks: Vec<String> = vec![];
    let mut total = 0;
    for (i, path) in inputs.iter().enumerate() {
        if prepare_ui {
            publish_json(
                &out,
                "progress.json",
                &serde_json::json!({"page":i+1,"pages":inputs.len(),"stage":"render"}),
            )?;
        }
        if is_pdf {
            let prefix = path.with_extension("");
            let result = background_command(poppler())
                .args([
                    "-r",
                    &dpi.to_string(),
                    "-f",
                    &(i + 1).to_string(),
                    "-l",
                    &(i + 1).to_string(),
                    "-singlefile",
                    "-png",
                ])
                .arg(&input)
                .arg(prefix)
                .output()?;
            if !result.status.success() {
                local_bail!(
                    "Page {}: {}",
                    "Strana {}: {}",
                    i + 1,
                    String::from_utf8_lossy(&result.stderr)
                );
            }
        }
        let mut decoded = image::ImageReader::open(path)?
            .with_guessed_format()?
            .decode()?;
        // Respect JPEG/TIFF EXIF orientation before measuring any corners.
        let mut reader = image::ImageReader::open(path)?
            .with_guessed_format()?
            .into_decoder()?;
        if let Ok(orientation) = image::ImageDecoder::orientation(&mut reader) {
            decoded.apply_orientation(orientation);
        }
        let img = decoded.to_rgb8();
        let mut photos = if let Some(p) = &saved {
            let pg = &p.pages[i];
            if pg.width != img.width() || pg.height != img.height() {
                local_bail!(
                    "Page {} dimensions do not match the plan. Use the original DPI.",
                    "Rozmery strany {} nezodpovedaju planu. Pouzi povodne DPI.",
                    i + 1
                );
            }
            pg.photos.clone()
        } else {
            detect(&img)
        };
        local_println!(
            "Page {}: {} photos",
            "Strana {}: {} fotiek",
            i + 1,
            photos.len()
        );
        let photo_count = photos.len();
        for (j, p) in photos.iter_mut().enumerate() {
            if prepare_ui {
                publish_json(
                    &out,
                    "progress.json",
                    &serde_json::json!({"page":i+1,"pages":inputs.len(),"photo":j+1,"photos":photo_count,"stage":"photo"}),
                )?;
            }
            let mut confident = true;
            if saved.is_none() {
                let preview = crop(&img, p)?;
                let (rotation, certain) = orienter.predict(&preview);
                p.rotation = rotation;
                confident = certain;
                if !confident {
                    orientation_checks.push(format!("strana_{:03}_fotka_{:02}.jpg", i + 1, j + 1));
                }
            }
            if saved.is_some() || prepare_ui {
                let photo = crop(&img, p)?;
                let name = format!("strana_{:03}_fotka_{:02}.jpg", i + 1, j + 1);
                let pending = out.join(format!("{name}.part"));
                {
                    let file = fs::File::create(&pending)?;
                    JpegEncoder::new_with_quality(file, 98)
                        .encode_image(&DynamicImage::ImageRgb8(photo))?;
                }
                fs::rename(pending, out.join(&name))?;
                if prepare_ui {
                    publish_json(
                        &out,
                        &format!("{name}.ready.json"),
                        &serde_json::json!({"name":name,"page":i+1,"photo":j+1,"uncertain":!confident}),
                    )?;
                }
                total += 1;
            }
        }
        let mut png = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img.clone()).write_to(&mut png, image::ImageFormat::Png)?;
        pages.push(Page {
            name: format!("{} {}", language::text("Page", "Strana"), i + 1),
            width: img.width(),
            height: img.height(),
            data: STANDARD.encode(png.into_inner()),
            photos,
        });
    }
    fs::write(
        out.join("orientacia-kontrola.json"),
        serde_json::to_vec(&orientation_checks)?,
    )?;
    let data = serde_json::to_string(&pages)?;
    let html = include_str!("review.html").replace("/*DATA*/[]", &data).replace("/*L10N*/{}",&serde_json::json!({"lang":language::code(),"messages":language::catalog()[language::code()]["review"]}).to_string());
    let review = out.join("kontrola.html");
    fs::write(&review, html)?;
    let absolute_review = fs::canonicalize(&review)?;
    local_println!("Review: {}", "Kontrola: {}", absolute_review.display());
    if cfg!(windows) && !no_open {
        let display_path = absolute_review
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_owned();
        let _ = background_command("powershell.exe")
            .env("FOTO_REVIEW", display_path)
            .args([
                "-NoProfile",
                "-Command",
                "Start-Process -FilePath $env:FOTO_REVIEW",
            ])
            .spawn();
    }
    if saved.is_some() {
        local_println!(
            "Saved {total} JPEG photos.",
            "Ulozenych {total} JPEG fotiek."
        );
    } else {
        local_println!(
            "Open kontrola.html to review crops and rotation, then download the ZIP or plan.",
            "Otvor kontrola.html, uprav vyrezy a otocenie, potom stiahni ZIP alebo plan."
        );
    }
    Ok(())
}
fn publish_json(out: &Path, name: &str, value: &serde_json::Value) -> Result<()> {
    let pending = out.join(format!("{name}.part"));
    fs::write(&pending, serde_json::to_vec(value)?)?;
    fs::rename(pending, out.join(name))?;
    Ok(())
}
// Console helpers must never create a terminal beside the application.
fn background_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}
fn launch_gui() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let script = temp.path().join("foto-ui.ps1");
    // Windows PowerShell requires a BOM for UTF-8 Slovak strings.
    let contents = format!(
        "\u{feff}$script:catalog=@'\n{}\n'@ | ConvertFrom-Json\n{}\n{}",
        include_str!("localization.json"),
        include_str!("localization.ps1"),
        include_str!("ui.ps1")
    );
    fs::write(&script, contents)?;
    let mut cmd = background_command("powershell.exe");
    cmd.args(["-NoProfile", "-STA", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(script)
        .env("FOTO_EXE", env::current_exe()?)
        .env("PHOTOSPLITTER_LANG", language::code());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.status()?;
    Ok(())
}
fn main() {
    if cfg!(windows) && env::args_os().len() == 1 {
        if let Err(e) = launch_gui() {
            eprintln!("{e:#}");
        }
        return;
    }
    if let Err(e) = run() {
        eprintln!("{}: {e:#}", language::text("Error", "Chyba"));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn helper_runs_without_a_console_window() {
        let probe = r#"Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class ConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); }'; [Console]::Write([ConsoleProbe]::GetConsoleWindow().ToInt64())"#;
        let result = background_command("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", probe])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "0");
    }
    #[test]
    fn separates_four_photos_and_ignores_blank() {
        let mut im = RgbImage::from_pixel(400, 600, Rgb([255, 255, 255]));
        for (x0, y0) in [(20, 10), (210, 10), (20, 270), (210, 270)] {
            for y in y0..y0 + 240 {
                for x in x0..x0 + 170 {
                    im.put_pixel(x, y, Rgb([70, 90, 80]));
                }
            }
        }
        assert_eq!(detect(&im).len(), 4);
        assert!(detect(&RgbImage::from_pixel(100, 100, Rgb([255, 255, 255]))).is_empty());
    }
    #[test]
    fn rotates_pixels_and_rejects_crossed_corners() {
        let im = RgbImage::from_fn(4, 3, |x, y| Rgb([(x * 40) as u8, (y * 60) as u8, 0]));
        let p = Photo {
            corners: [[0., 0.], [3., 0.], [3., 2.], [0., 2.]],
            rotation: 90,
        };
        let got = crop(&im, &p).unwrap();
        assert_eq!(got.get_pixel(got.width() - 1, 0), im.get_pixel(0, 0));
        let bad = Photo {
            corners: [[0., 0.], [3., 2.], [3., 0.], [0., 2.]],
            rotation: 0,
        };
        assert!(crop(&im, &bad).is_err());
    }
}
