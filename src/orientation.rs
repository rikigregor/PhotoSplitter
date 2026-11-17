use anyhow::Result;
use image::{DynamicImage, RgbImage};
use std::path::Path;

pub struct Orienter {
    detector: Box<dyn rustface::Detector>,
}
impl Orienter {
    pub fn new(dir: &Path) -> Result<Self> {
        let path = dir.join("orientation-face-model.bin");
        std::fs::write(&path, include_bytes!("face-model.bin"))?;
        let mut detector = rustface::create_detector(path.to_str().unwrap())?;
        detector.set_min_face_size(20);
        detector.set_score_thresh(2.8);
        detector.set_pyramid_scale_factor(0.8);
        detector.set_slide_window_step(4, 4);
        Ok(Self { detector })
    }
    pub fn predict(&mut self, src: &RgbImage) -> (u16, bool) {
        let small = DynamicImage::ImageRgb8(src.clone())
            .thumbnail(800, 800)
            .to_luma8();
        let mut scores = [0.0f64; 4];
        for (i, score) in scores.iter_mut().enumerate() {
            let im = match i {
                1 => image::imageops::rotate90(&small),
                2 => image::imageops::rotate180(&small),
                3 => image::imageops::rotate270(&small),
                _ => small.clone(),
            };
            let input = rustface::ImageData::new(im.as_raw(), im.width(), im.height());
            let faces = self.detector.detect(&input);
            let mut found: Vec<f64> = faces.iter().map(|f| f.score()).collect();
            found.sort_by(|a, b| b.total_cmp(a));
            *score = found.iter().take(3).sum();
        }
        let mut order = [0usize, 1, 2, 3];
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
        let best = order[0];
        let second = scores[order[1]];
        let certain = scores[best] >= 3.5 && scores[best] > second * 1.35 + 0.5;
        // A weak or ambiguous detection is not evidence for turning a photo.
        if certain {
            ((best * 90) as u16, true)
        } else {
            (0, false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blank_has_no_orientation_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let mut model = Orienter::new(dir.path()).unwrap();
        let blank = RgbImage::from_pixel(160, 120, image::Rgb([240, 240, 240]));
        assert_eq!(model.predict(&blank), (0, false));
    }
    #[test]
    #[ignore = "local photograph fixture supplied through FOTO_ORIENTATION_TEST_IMAGE"]
    fn all_four_rotations_of_local_portrait() {
        let path = std::env::var("FOTO_ORIENTATION_TEST_IMAGE").unwrap();
        let src = image::open(path).unwrap().to_rgb8();
        let dir = tempfile::tempdir().unwrap();
        let mut model = Orienter::new(dir.path()).unwrap();
        for angle in [0u16, 90, 180, 270] {
            let im = match angle {
                90 => image::imageops::rotate90(&src),
                180 => image::imageops::rotate180(&src),
                270 => image::imageops::rotate270(&src),
                _ => src.clone(),
            };
            let (correction, certain) = model.predict(&im);
            assert!(certain, "uncertain at {angle}");
            assert_eq!((angle + correction) % 360, 0, "wrong at {angle}");
        }
    }
}
