//! Font configuration: the text family + math font used to render a figure.
//!
//! A [`FontSet`] bundles a text font (regular/bold/italic, drawn as glyph
//! outlines) and an OpenType **MATH** font used by the ReX engine to typeset
//! `$…$` spans. The default is **STIX Two** — a LaTeX-like serif paired with STIX
//! Two Math — giving plots a publication ("LaTeX") look out of the box. The
//! matplotlib sans-serif look is available as `FontSet::dejavu`, and any
//! TrueType/OpenType font can be supplied with `FontSet::from_font` /
//! `FontSet::from_path`.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::{GlyphMetrics, Metrics};
use skrifa::outline::{DrawSettings, OutlineGlyphCollection, OutlinePen};
use skrifa::raw::TableProvider;
use skrifa::raw::tables::kern::Subtable as KernSubtable;
use skrifa::{FontRef, GlyphId, MetadataProvider};

use crate::error::{Error, Result};
use crate::text::FontStyle;

// Bundled OFL fonts (see the matching `*-LICENSE.txt` in `assets/`).
static STIX_REGULAR: &[u8] = include_bytes!("../assets/STIXTwoText-Regular.otf");
static STIX_BOLD: &[u8] = include_bytes!("../assets/STIXTwoText-Bold.otf");
static STIX_ITALIC: &[u8] = include_bytes!("../assets/STIXTwoText-Italic.otf");
#[cfg(feature = "math")]
static STIX_MATH: &[u8] = include_bytes!("../assets/STIXTwoMath-Regular.otf");
static DEJAVU_REGULAR: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");
static DEJAVU_BOLD: &[u8] = include_bytes!("../assets/DejaVuSans-Bold.ttf");
static DEJAVU_OBLIQUE: &[u8] = include_bytes!("../assets/DejaVuSans-Oblique.ttf");

/// The fonts used to draw a figure: a text family (regular/bold/italic) and an
/// OpenType MATH font for `$…$` spans.
///
/// Defaults to [`FontSet::stix`] (a LaTeX-like serif). It is cheap to clone
/// (the parsed faces are shared behind an `Arc`), so it lives on
/// [`Style`](crate::Style) and is carried by every panel.
///
/// # Examples
/// ```no_run
/// use oxiplot::{Axes, FontSet};
/// // The matplotlib sans-serif look:
/// let mut ax = Axes::new();
/// ax.fonts(FontSet::dejavu());
/// // A custom text font from disk (STIX Two Math for the `$…$` spans):
/// let custom = FontSet::from_path("/path/to/MyFont.otf").unwrap();
/// ax.fonts(custom);
/// ```
#[derive(Clone)]
pub struct FontSet {
    inner: Arc<Inner>,
}

struct Inner {
    /// The font files, checked to parse; [`FontSet::face`] reads them.
    regular: Vec<u8>,
    bold: Vec<u8>,
    italic: Vec<u8>,
    /// Raw bytes of the OpenType MATH font (parsed per math run by ReX).
    #[cfg(feature = "math")]
    math: Vec<u8>,
}

impl std::fmt::Debug for FontSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontSet").finish_non_exhaustive()
    }
}

impl Default for FontSet {
    fn default() -> Self {
        FontSet::stix()
    }
}

impl FontSet {
    /// The default LaTeX-like set: **STIX Two Text** + **STIX Two Math**.
    ///
    /// The parsed faces are built once and shared, so repeated calls are cheap.
    #[must_use]
    pub fn stix() -> FontSet {
        static CELL: OnceLock<FontSet> = OnceLock::new();
        CELL.get_or_init(|| from_static(STIX_REGULAR, STIX_BOLD, STIX_ITALIC))
            .clone()
    }

    /// The matplotlib look: **DejaVu Sans** text with STIX Two Math for `$…$`.
    #[must_use]
    pub fn dejavu() -> FontSet {
        static CELL: OnceLock<FontSet> = OnceLock::new();
        CELL.get_or_init(|| from_static(DEJAVU_REGULAR, DEJAVU_BOLD, DEJAVU_OBLIQUE))
            .clone()
    }

    /// A custom text font (used for regular/bold/italic) with the default STIX
    /// Two Math for `$…$` spans.
    ///
    /// # Errors
    /// If the bytes are not a valid TrueType/OpenType font.
    pub fn from_font(text: &[u8]) -> Result<FontSet> {
        Ok(FontSet {
            inner: Arc::new(Inner {
                regular: load_text_font(text)?,
                bold: load_text_font(text)?,
                italic: load_text_font(text)?,
                #[cfg(feature = "math")]
                math: STIX_MATH.to_vec(),
            }),
        })
    }

    /// A custom text font plus a custom math font (an OpenType font with a `MATH`
    /// table, e.g. STIX Two Math, XITS Math, Latin Modern Math).
    ///
    /// # Errors
    /// If either font cannot be parsed (the math font must have a `MATH` table).
    ///
    /// A build without the `math` feature lays `$…$` spans out as plain text and
    /// only checks that the math font parses.
    pub fn from_fonts(text: &[u8], math: &[u8]) -> Result<FontSet> {
        // Validate the math font parses as a face (ReX re-reads the bytes later).
        FontRef::new(math).map_err(|e| Error::Font(format!("invalid math font: {e}")))?;
        Ok(FontSet {
            inner: Arc::new(Inner {
                regular: load_text_font(text)?,
                bold: load_text_font(text)?,
                italic: load_text_font(text)?,
                #[cfg(feature = "math")]
                math: math.to_vec(),
            }),
        })
    }

    /// Load a custom text font from a file (STIX Two Math for `$…$` spans).
    ///
    /// # Errors
    /// On I/O failure or if the font cannot be parsed.
    pub fn from_path(text: impl AsRef<Path>) -> Result<FontSet> {
        let bytes = std::fs::read(text)?;
        Self::from_font(&bytes)
    }

    /// Load a custom text font and a custom math font from files.
    ///
    /// # Errors
    /// On I/O failure or if either font cannot be parsed.
    pub fn from_paths(text: impl AsRef<Path>, math: impl AsRef<Path>) -> Result<FontSet> {
        let text = std::fs::read(text)?;
        let math = std::fs::read(math)?;
        Self::from_fonts(&text, &math)
    }

    /// The text face for `style`, ready to measure and draw.
    pub(crate) fn face(&self, style: FontStyle) -> Face<'_> {
        let bytes = match style {
            FontStyle::Regular => &self.inner.regular,
            FontStyle::Bold => &self.inner.bold,
            FontStyle::Italic => &self.inner.italic,
        };
        Face::new(FontRef::new(bytes).expect("checked when the FontSet was built"))
    }

    #[cfg(feature = "math")]
    pub(crate) fn math_bytes(&self) -> &[u8] {
        &self.inner.math
    }
}

fn load_text_font(bytes: &[u8]) -> Result<Vec<u8>> {
    let font = FontRef::new(bytes).map_err(|e| Error::Font(format!("invalid text font: {e}")))?;
    font.head()
        .map_err(|e| Error::Font(format!("invalid text font: {e}")))?;
    Ok(bytes.to_vec())
}

/// A parsed text face: the character map, horizontal metrics, `kern` pairs
/// and outlines, all in design units (y up).
pub(crate) struct Face<'a> {
    charmap: Charmap<'a>,
    metrics: GlyphMetrics<'a>,
    outlines: OutlineGlyphCollection<'a>,
    /// Horizontal, non-variable `kern` subtables, first match wins.
    kern: Vec<KernSubtable<'a>>,
    /// Design units per em.
    pub(crate) units_per_em: f32,
    /// Ascent above the baseline, in design units (positive).
    pub(crate) ascent: f32,
    /// Descent, in design units (negative below the baseline).
    pub(crate) descent: f32,
}

impl<'a> Face<'a> {
    fn new(font: FontRef<'a>) -> Face<'a> {
        let line = Metrics::new(&font, Size::unscaled(), LocationRef::default());
        let kern = font
            .kern()
            .map(|k| {
                k.subtables()
                    .flatten()
                    .filter(|st| st.is_horizontal() && !st.is_variable())
                    .collect()
            })
            .unwrap_or_default();
        Face {
            charmap: font.charmap(),
            metrics: font.glyph_metrics(Size::unscaled(), LocationRef::default()),
            outlines: font.outline_glyphs(),
            kern,
            units_per_em: if line.units_per_em == 0 {
                2048.0
            } else {
                f32::from(line.units_per_em)
            },
            ascent: line.ascent,
            descent: line.descent,
        }
    }

    /// The glyph for `c`, or `.notdef` (glyph 0) if the font has none.
    pub(crate) fn glyph_id(&self, c: char) -> GlyphId {
        self.charmap.map(c).unwrap_or(GlyphId::NOTDEF)
    }

    /// The advance width of `id`, in design units.
    pub(crate) fn h_advance(&self, id: GlyphId) -> f32 {
        self.metrics.advance_width(id).unwrap_or_default()
    }

    /// The `kern` table's adjustment between `left` and `right`, in design
    /// units (zero without one).
    pub(crate) fn kern(&self, left: GlyphId, right: GlyphId) -> f32 {
        use skrifa::raw::tables::kern::SubtableKind;
        self.kern
            .iter()
            .find_map(|st| match st.kind().ok()? {
                SubtableKind::Format0(t) => t.kerning(left, right),
                SubtableKind::Format2(t) => t.kerning(left, right),
                SubtableKind::Format3(t) => t.kerning(left, right),
                SubtableKind::Format1(_) => None,
            })
            .map_or(0.0, |k| k as f32)
    }

    /// Draw the outline of `id` into `pen`, in design units (y up). `false`
    /// if the glyph has no outline.
    pub(crate) fn draw(&self, id: GlyphId, pen: &mut impl OutlinePen) -> bool {
        let Some(glyph) = self.outlines.get(id) else {
            return false;
        };
        glyph
            .draw(
                DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
                pen,
            )
            .is_ok()
    }
}

/// A bundled text family, with STIX Two Math for `$…$` spans.
fn from_static(r: &[u8], b: &[u8], i: &[u8]) -> FontSet {
    let load = |bytes: &[u8]| load_text_font(bytes).expect("bundled font is valid");
    FontSet {
        inner: Arc::new(Inner {
            regular: load(r),
            bold: load(b),
            italic: load(i),
            #[cfg(feature = "math")]
            math: STIX_MATH.to_vec(),
        }),
    }
}
