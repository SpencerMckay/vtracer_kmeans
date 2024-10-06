use std::fmt;
use visioncortex::{Color, CompoundPath, PointF64};

#[repr(C)] 
pub struct SvgFile {
    pub paths: Vec<SvgPath>,
    pub width: usize,
    pub height: usize,
    pub path_precision: Option<u32>,
}

#[repr(C)]
pub struct SvgPath {
    pub path: CompoundPath,
    pub color: Color,
}

impl SvgFile {
    #[no_mangle]
    pub extern "C" fn new(width: usize, height: usize, path_precision: Option<u32>) -> Self {
        SvgFile {
            paths: vec![],
            width,
            height,
            path_precision,
        }
    }

    #[no_mangle]
    pub extern "C" fn add_path(&mut self, path: CompoundPath, color: Color) {
        self.paths.push(SvgPath { path, color })
    }
}

impl fmt::Display for SvgFile {
    #[no_mangle]
    extern "C" fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
        writeln!(
            f,
            r#"<!-- Generator: visioncortex VTracer {} -->"#,
            env!("CARGO_PKG_VERSION")
        )?;
        writeln!(
            f,
            r#"<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
            self.width, self.height
        )?;

        for path in &self.paths {
            path.fmt_with_precision(f, self.path_precision)?;
        }

        writeln!(f, "</svg>")
    }
}

impl fmt::Display for SvgPath {
    #[no_mangle]
    extern "C"  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_with_precision(f, None)
    }
}

impl SvgPath {
    #[no_mangle]
    pub extern "C" fn fmt_with_precision(&self, f: &mut fmt::Formatter, precision: Option<u32>) -> fmt::Result {
        let (string, offset) = self
            .path
            .to_svg_string(true, PointF64::default(), precision);
        writeln!(
            f,
            "<path d=\"{}\" fill=\"{}\" transform=\"translate({},{})\"/>",
            string,
            self.color.to_hex_string(),
            offset.x,
            offset.y
        )
    }
}
