//! Tests für Zuschnitt, Rendern, Speichern, Text und Begradigen.

use super::*;

#[cfg(test)]
mod basics {
    use super::*;

    #[test]
    fn explains_errors_in_german() {
        assert!(
            explain_error("Format error decoding Jpeg: unexpected EOF").contains("unvollständig")
        );
        assert!(explain_error("The image format could not be determined").contains("beschädigt"));
        assert!(explain_error("weird").starts_with("Fehler"));
    }

    #[test]
    fn smart_center_finds_detail() {
        // breites, ruhiges Foto mit Muster ganz rechts
        let img = RgbImage::from_fn(400, 150, |x, y| {
            if x > 300 && (x / 6 + y / 6) % 2 == 0 {
                image::Rgb([250, 250, 250])
            } else {
                image::Rgb([40, 60, 80])
            }
        });
        let (cx, cy) = smart_center(&img, 1.0);
        assert!(cx > 0.65, "cx = {cx}");
        assert!((cy - 0.5).abs() < 1e-3);
        let flat = RgbImage::from_pixel(400, 150, image::Rgb([90, 90, 90]));
        let (cx, _) = smart_center(&flat, 1.0);
        assert!((cx - 0.5).abs() < 0.05, "cx = {cx}");
    }

    #[test]
    #[ignore]
    fn smart_center_on_file() {
        let p = std::env::var("BRS_SMART").unwrap();
        let img = image::open(p).unwrap().to_rgb8();
        let small = resize(&img, 400, 400 * img.height() / img.width(), true);
        println!("16:9 {:?}", smart_center(&small, 16.0 / 9.0));
        println!("9:16 {:?}", smart_center(&small, 9.0 / 16.0));
    }

    #[test]
    fn ratios() {
        assert!((parse_ratio("16:9").unwrap() - 16.0 / 9.0).abs() < 1e-6);
        assert!((parse_ratio("1280 x 800").unwrap() - 1.6).abs() < 1e-6);
        assert!((parse_ratio("1,5").unwrap() - 1.5).abs() < 1e-6);
        assert!(parse_ratio("abc").is_none());
        assert!(parse_ratio("16:0").is_none());
    }

    #[test]
    fn crop_centered() {
        let mut e = Edit::default();
        let (x, y, w, h) = crop_rect(4000.0, 3000.0, 16.0 / 9.0, &mut e);
        assert_eq!((x, w), (0.0, 4000.0));
        assert!((h - 2250.0).abs() < 0.01 && (y - 375.0).abs() < 0.01);
        e.cy = 0.0; // ganz nach oben schieben
        let (_, y, _, _) = crop_rect(4000.0, 3000.0, 16.0 / 9.0, &mut e);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn exif_len() {
        assert_eq!(exif_block(NaiveDateTime::default()).len(), 6 + 108);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    fn sample(dir: &Path, w: u32, h: u32) -> PathBuf {
        let mut img = RgbImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            // Rauschen, damit JPEG nicht winzig wird
            let n = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503)) as u8;
            *p = image::Rgb([n, (x / 7) as u8, (y / 5) as u8]);
        }
        let p = dir.join("probe.jpg");
        img.save(&p).unwrap();
        p
    }

    #[test]
    fn limits_and_folders() {
        let dir = std::env::temp_dir().join(format!("brs_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let src = sample(&dir, 4000, 3000);
        let cfg = OutputSettings {
            ratio_quer: 16.0 / 9.0,
            ratio_hoch: 9.0 / 16.0,
            max_res: Some((1920, 1080)),
            max_mb: 0.3,
            quality: 92,
            fill_bg: FillBg::default(),
        };
        let t = std::time::Instant::now();
        let out = save(&src, &Edit::default(), &cfg, None, &dir.join("out"), None).unwrap();
        eprintln!("speichern: {:?}", t.elapsed());
        assert!(out.starts_with(dir.join("out").join("Quer")));
        let meta = fs::metadata(&out).unwrap();
        assert!(
            meta.len() <= 300 * 1024 * 1024 / 1000,
            "zu groß: {}",
            meta.len()
        );
        let img = image::open(&out).unwrap();
        assert!(img.width() <= 1920 && img.height() <= 1080);
        // Hochkant erzwingen
        let e = Edit {
            orient: Some(Orient::Hochkant),
            ..Default::default()
        };
        let out2 = save(&src, &e, &cfg, None, &dir.join("out"), None).unwrap();
        assert!(out2.starts_with(dir.join("out").join("Hochkant")));
        let img2 = image::open(&out2).unwrap();
        assert!(img2.height() > img2.width() && img2.height() <= 1920);
        // neu speichern mit `replace`: gleicher Name; ohne: zweiter Name daneben
        let again = save(&src, &e, &cfg, None, &dir.join("out"), Some(&out2)).unwrap();
        assert_eq!(again, out2);
        let other = save(&src, &e, &cfg, None, &dir.join("out"), None).unwrap();
        assert_ne!(other, out2);
        assert!(other.to_string_lossy().contains("_2"));
        let _ = fs::remove_file(&other);
        // `replace` in der anderen Ausrichtung wird nicht überschrieben
        let q = save(
            &src,
            &Edit::default(),
            &cfg,
            None,
            &dir.join("out"),
            Some(&out2),
        )
        .unwrap();
        assert!(q.starts_with(dir.join("out").join("Quer")));
        assert!(out2.exists());
        // ganzes Foto auf unscharfem Hintergrund, Text unten links
        let e = Edit {
            fill: true,
            orient: Some(Orient::Quer),
            ..Default::default()
        };
        let tall = {
            let p = dir.join("hoch.jpg");
            image::imageops::rotate90(&image::open(&src).unwrap().into_rgb8())
                .save(&p)
                .unwrap();
            p
        };
        let ov = Overlay {
            lines: vec!["15.07.2024 · Italien".into()],
            look: Default::default(),
        };
        let out3 = save(&tall, &e, &cfg, Some(&ov), &dir.join("out"), None).unwrap();
        assert!(out3.starts_with(dir.join("out").join("Quer")));
        let img3 = image::open(&out3).unwrap();
        assert!(img3.width() > img3.height());
        assert!(((img3.width() as f32 / img3.height() as f32) - 16.0 / 9.0).abs() < 0.01);
        let t = std::time::Instant::now();
        load_preview(&src, 2048).unwrap();
        eprintln!("vorschau: {:?}", t.elapsed());
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod overlay_tests {
    use super::*;

    #[test]
    fn styles_draw_something() {
        for (style, look) in crate::textstyle::presets().into_iter().enumerate() {
            let mut img = RgbImage::from_pixel(800, 450, image::Rgb([90, 120, 160]));
            let o = Overlay {
                lines: vec!["15. Juli 2024  ·  Gardasee".into(), "Baden mit Oma".into()],
                look,
            };
            draw_overlay(&mut img, &o);
            let changed = img.pixels().filter(|p| p.0 != [90, 120, 160]).count();
            assert!(changed > 1000, "Stil {style}: nichts gezeichnet");
            if let Some(dir) = std::env::var_os("BRS_TEST_OUT") {
                img.save(PathBuf::from(dir).join(format!("stil{style}.png")))
                    .unwrap();
            }
        }
    }
}

#[cfg(test)]
mod fill_look {
    use super::*;
    /// Nur von Hand: Beispielbilder für die Hintergründe (BRS_FILL_SAMPLE=foto.jpg).
    #[test]
    #[ignore]
    fn samples() {
        let Ok(src) = std::env::var("BRS_FILL_SAMPLE") else {
            return;
        };
        let img = rotate(to_rgb(decode(Path::new(&src)).unwrap().img), 0);
        let img = resize(&img, img.width() / 2, img.height() / 2, false);
        for (i, bg) in [(0u8, 0u8), (0, 40), (0, 100), (1, 0)].iter().enumerate() {
            let out = fill_canvas(
                &img,
                16.0 / 9.0,
                Some((1280, 720)),
                &Edit::default(),
                FillBg {
                    kind: bg.0,
                    blur: bg.1,
                },
            );
            out.save(format!("/tmp/claude-0/run/v34/fill{i}.png"))
                .unwrap();
        }
    }
}

#[cfg(test)]
mod straighten_tests {
    use super::*;

    #[test]
    fn finds_and_fixes_tilted_horizon() {
        // Horizont, der nach rechts um 3° abfällt (im Uhrzeigersinn geneigt)
        let t = 3f32.to_radians().tan();
        let img = RgbImage::from_fn(800, 500, |x, y| {
            let horizon = 250.0 + (x as f32 - 400.0) * t;
            if (y as f32) < horizon {
                image::Rgb([120, 170, 230])
            } else {
                image::Rgb([60, 90, 40])
            }
        });
        let tilt = suggest_angle(&img).expect("Vorschlag");
        assert!((tilt - 3.0).abs() < 0.5, "erkannt: {tilt}");
        let fixed = straighten(&img, -tilt);
        assert_eq!(fixed.dimensions(), img.dimensions());
        let again = suggest_angle(&fixed);
        assert!(again.is_none_or(|a| a.abs() < 0.6), "danach: {again:?}");
        // Ecken sind gefüllt (kein Schwarz)
        assert!(fixed.get_pixel(0, 0).0 != [0, 0, 0]);
        // Licht: neutral ändert nichts, heller macht heller
        let mut a = img.clone();
        apply_tone(&mut a, Tone::default());
        assert_eq!(a, img);
        apply_tone(
            &mut a,
            Tone {
                auto: false,
                bright: 30,
                contrast: 0,
            },
        );
        assert!(a.get_pixel(10, 400).0[0] > img.get_pixel(10, 400).0[0]);
    }
}

#[cfg(test)]
mod straighten_samples {
    /// Nur von Hand: Vorschläge für alle Bilder in BRS_TILT_DIR ausgeben.
    #[test]
    #[ignore]
    fn print() {
        let Ok(dir) = std::env::var("BRS_TILT_DIR") else {
            return;
        };
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if let Ok((img, ..)) = super::load_preview(&p, 1200) {
                let t = std::time::Instant::now();
                eprintln!(
                    "{:?}: {:?} ({:?})",
                    p.file_name().unwrap(),
                    super::suggest_angle(&img),
                    t.elapsed()
                );
            }
        }
    }
}
