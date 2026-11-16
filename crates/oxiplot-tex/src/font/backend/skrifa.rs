//! A [`MathFont`](crate::font::MathFont) over [skrifa] and its re-exported
//! [read-fonts](skrifa::raw) (Google Fonts' "fontations"): the MATH table for
//! layout, GSUB's `ssty` feature for script-size glyphs, and skrifa's outlines
//! for drawing.
//!
//! This replaces upstream ReX's `ttf-parser` backend; it gives the same values
//! (same tables, same bounding boxes), so layouts do not change.

use skrifa::instance::Size;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::tables::gsub::{AlternateSubst, Gsub, SingleSubst, SubstitutionSubtables};
use skrifa::raw::tables::math::{
    GlyphPartRecord, Math, MathConstants, MathKernCorner, MathVariants, StretchAxis,
};
use skrifa::raw::types::Tag;
use skrifa::raw::TableProvider;
use skrifa::{FontRef, MetadataProvider};

use crate::dimensions::units::Ratio;
use crate::dimensions::units::{Em, FUnit};
use crate::dimensions::Unit;
use crate::{
    error::FontError,
    font::{
        common::{GlyphId, GlyphInstruction, ScriptLevel},
        Direction, FontConstants, Glyph, VariantGlyph,
    },
};

/// A math font: a parsed font with a MATH table, plus the tables layout reads
/// on every glyph.
pub struct SkrifaMathFont<'a> {
    font: FontRef<'a>,
    math: Math<'a>,
    gsub: Option<Gsub<'a>>,
    charmap: skrifa::charmap::Charmap<'a>,
    outlines: skrifa::outline::OutlineGlyphCollection<'a>,
    metrics: skrifa::metrics::GlyphMetrics<'a>,
    units_per_em: u16,
}

impl<'a> SkrifaMathFont<'a> {
    /// Parse `data` (a TrueType/OpenType file, the first face of a
    /// collection).
    ///
    /// # Errors
    ///
    /// [`FontError::Unreadable`] if the data is not a font,
    /// [`FontError::NoMATHTable`] if it has no usable MATH table.
    pub fn new(data: &'a [u8]) -> Result<Self, FontError> {
        let font = FontRef::new(data).map_err(|e| FontError::Unreadable(e.to_string()))?;
        Self::from_font(font)
    }

    /// Wrap an already parsed font.
    ///
    /// # Errors
    ///
    /// [`FontError::NoMATHTable`] if it has no usable MATH table.
    pub fn from_font(font: FontRef<'a>) -> Result<Self, FontError> {
        let math = font.math().map_err(|_| FontError::NoMATHTable)?;
        let units_per_em = font
            .head()
            .map_err(|e| FontError::Unreadable(e.to_string()))?
            .units_per_em();
        Ok(Self {
            gsub: font.gsub().ok(),
            charmap: font.charmap(),
            outlines: font.outline_glyphs(),
            metrics: font.glyph_metrics(Size::unscaled(), skrifa::instance::LocationRef::default()),
            units_per_em,
            math,
            font,
        })
    }

    /// The wrapped font.
    pub fn font(&self) -> &FontRef<'a> {
        &self.font
    }

    /// Design units per em.
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    /// Draw a glyph's outline, in design units (y up), into `pen`. `false` if
    /// the font has no outline for it.
    pub fn draw_glyph(&self, gid: GlyphId, pen: &mut impl OutlinePen) -> bool {
        let Some(glyph) = self.outlines.get(sk_gid(gid)) else {
            return false;
        };
        glyph
            .draw(
                DrawSettings::unhinted(Size::unscaled(), skrifa::instance::LocationRef::default()),
                pen,
            )
            .is_ok()
    }

    /// The bounding box of every point of the outline, control points
    /// included, as ttf-parser (and ReX's layout tests) measured it. `None`
    /// for a glyph without an outline.
    fn outline_bbox(&self, gid: GlyphId) -> Option<(i16, i16, i16, i16)> {
        let mut pen = BBoxPen::default();
        if !self.draw_glyph(gid, &mut pen) || !pen.seen {
            return None;
        }
        let to = |v: f32| -> Option<i16> {
            (v >= f32::from(i16::MIN) && v <= f32::from(i16::MAX)).then(|| v as i16)
        };
        Some((to(pen.x_min)?, to(pen.y_min)?, to(pen.x_max)?, to(pen.y_max)?))
    }

    fn constants_table(&self) -> Option<MathConstants<'a>> {
        self.math.math_constants().ok()
    }

    fn variants(&self) -> Option<MathVariants<'a>> {
        self.math.math_variants().ok()
    }

    fn safe_italics(&self, gid: GlyphId) -> Option<i16> {
        let info = self.math.math_glyph_info().ok()?;
        let table = info.math_italics_correction_info()?.ok()?;
        Some(table.correction(sk_gid(gid))? as i16)
    }

    fn safe_attachment(&self, gid: GlyphId) -> Option<i16> {
        let info = self.math.math_glyph_info().ok()?;
        let table = info.math_top_accent_attachment()?.ok()?;
        Some(table.attachment(sk_gid(gid))? as i16)
    }

    fn safe_constants(&self, font_units_to_em: Unit<Ratio<Em, FUnit>>) -> Option<FontConstants> {
        let c = self.constants_table()?;
        let em = |v: f64| -> Unit<Em> { Unit::<FUnit>::new(v) * font_units_to_em };
        let v = |r: &skrifa::raw::tables::math::MathValueRecord| em(f64::from(r.value().to_i16()));

        Some(FontConstants {
            // As upstream ReX: `subscript_shift_down` reads `subscriptTopMax`.
            // Kept so layouts do not change; see the oxiplot changelog.
            subscript_shift_down: v(c.subscript_top_max()),
            subscript_top_max: v(c.subscript_top_max()),
            subscript_baseline_drop_min: v(c.subscript_baseline_drop_min()),

            superscript_baseline_drop_max: v(c.superscript_baseline_drop_max()),
            superscript_bottom_min: v(c.superscript_bottom_min()),
            superscript_shift_up_cramped: v(c.superscript_shift_up_cramped()),
            superscript_shift_up: v(c.superscript_shift_up()),
            sub_superscript_gap_min: v(c.sub_superscript_gap_min()),

            upper_limit_baseline_rise_min: v(c.upper_limit_baseline_rise_min()),
            upper_limit_gap_min: v(c.upper_limit_gap_min()),
            lower_limit_gap_min: v(c.lower_limit_gap_min()),
            lower_limit_baseline_drop_min: v(c.lower_limit_baseline_drop_min()),

            fraction_rule_thickness: v(c.fraction_rule_thickness()),
            fraction_numerator_display_style_shift_up: v(c.fraction_numerator_display_style_shift_up()),
            fraction_denominator_display_style_shift_down: v(c.fraction_denominator_display_style_shift_down()),
            fraction_num_display_style_gap_min: v(c.fraction_num_display_style_gap_min()),
            fraction_denom_display_style_gap_min: v(c.fraction_denom_display_style_gap_min()),
            fraction_numerator_shift_up: v(c.fraction_numerator_shift_up()),
            fraction_denominator_shift_down: v(c.fraction_denominator_shift_down()),
            fraction_numerator_gap_min: v(c.fraction_numerator_gap_min()),
            fraction_denominator_gap_min: v(c.fraction_denominator_gap_min()),

            axis_height: v(c.axis_height()),
            accent_base_height: v(c.accent_base_height()),

            delimited_sub_formula_min_height: em(f64::from(c.delimited_sub_formula_min_height().to_u16())),

            display_operator_min_height: em(f64::from(c.display_operator_min_height().to_u16())),

            radical_display_style_vertical_gap: v(c.radical_display_style_vertical_gap()),
            radical_vertical_gap: v(c.radical_vertical_gap()),
            radical_rule_thickness: v(c.radical_rule_thickness()),
            radical_extra_ascender: v(c.radical_extra_ascender()),
            radical_kern_before_degree: v(c.radical_kern_before_degree()),
            radical_kern_after_degree: v(c.radical_kern_after_degree()),
            radical_degree_bottom_raise_percent: c.radical_degree_bottom_raise_percent(),

            stack_display_style_gap_min: v(c.stack_display_style_gap_min()),
            stack_top_display_style_shift_up: v(c.stack_top_display_style_shift_up()),
            stack_top_shift_up: v(c.stack_top_shift_up()),
            stack_bottom_shift_down: v(c.stack_bottom_shift_down()),
            stack_gap_min: v(c.stack_gap_min()),

            delimiter_factor: 0.901,
            delimiter_short_fall: Unit::<Em>::new(0.1),
            null_delimiter_space: Unit::<Em>::new(0.1),

            underbar_vertical_gap: v(c.underbar_vertical_gap()),
            underbar_rule_thickness: v(c.underbar_rule_thickness()),
            underbar_extra_descender: v(c.underbar_extra_descender()),

            script_percent_scale_down: 0.01 * f64::from(c.script_percent_scale_down()),
            script_script_percent_scale_down: 0.01 * f64::from(c.script_script_percent_scale_down()),
        })
    }

    /// The stretched form of `gid` along `axis` covering `size` design units:
    /// the first pre-made variant that is large enough, else an assembly of
    /// parts, else the largest variant.
    fn variant(&self, gid: GlyphId, size: Unit<FUnit>, axis: StretchAxis) -> VariantGlyph {
        // NOTE: an adaptation of the corresponding code in the crate "font".
        let Some(variants) = self.variants() else {
            return VariantGlyph::Replacement(gid);
        };
        // If the font does not specify a construction for the glyph, it is used as is.
        let Some(construction) = variants.glyph_construction(sk_gid(gid), axis) else {
            return VariantGlyph::Replacement(gid);
        };

        // Use the first replacement glyph at least as large as the demanded size.
        let records = construction.math_glyph_variant_records();
        let wanted = size.unitless(FUnit);
        for record in records {
            if record.advance_measurement().to_u16() >= wanted as u16 {
                return VariantGlyph::Replacement(GlyphId::from(record.variant_glyph().to_u16()));
            }
        }

        // Otherwise assemble it from parts, or fall back to the largest variant.
        let largest = records
            .last()
            .map(|v| GlyphId::from(v.variant_glyph().to_u16()))
            .unwrap_or(gid);
        let Some(Ok(assembly)) = construction.glyph_assembly() else {
            return VariantGlyph::Replacement(largest);
        };

        let size = wanted.ceil() as u32;
        // We aim for a construction where overlap between adjacent segments is the same.
        // Inspired by https://frederic-wang.fr/opentype-math-in-harfbuzz.html
        let instructions = construct_glyphs(
            variants.min_connector_overlap().to_u16().into(),
            assembly.part_records(),
            size,
        );
        let direction = match axis {
            StretchAxis::Horizontal => Direction::Horizontal,
            StretchAxis::Vertical => Direction::Vertical,
        };
        VariantGlyph::Constructable(direction, instructions)
    }
}

impl crate::font::MathFont for SkrifaMathFont<'_> {
    fn italics(&self, glyph_id: GlyphId) -> i16 {
        self.safe_italics(glyph_id).unwrap_or_default()
    }

    fn attachment(&self, glyph_id: GlyphId) -> i16 {
        self.safe_attachment(glyph_id).unwrap_or_default()
    }

    fn constants(&self, font_units_to_em: Unit<Ratio<Em, FUnit>>) -> FontConstants {
        self.safe_constants(font_units_to_em)
            .expect("the MATH table has a MathConstants subtable")
    }

    fn horz_variant(&self, gid: GlyphId, width: Unit<FUnit>) -> VariantGlyph {
        self.variant(gid, width, StretchAxis::Horizontal)
    }

    fn vert_variant(&self, gid: GlyphId, height: Unit<FUnit>) -> VariantGlyph {
        self.variant(gid, height, StretchAxis::Vertical)
    }

    fn glyph_index(&self, codepoint: char) -> Option<GlyphId> {
        let gid = self.charmap.map(codepoint)?;
        Some(GlyphId::from(u16::try_from(gid.to_u32()).ok()?))
    }

    fn glyph_from_gid<'f>(&'f self, gid: GlyphId) -> Result<Glyph<'f, Self>, FontError> {
        let id = sk_gid(gid);
        let (x_min, y_min, x_max, y_max) =
            self.outline_bbox(gid).ok_or(FontError::MissingGlyphGID(gid))?;
        let advance = self
            .metrics
            .advance_width(id)
            .ok_or(FontError::MissingGlyphGID(gid))?;
        let lsb = self
            .metrics
            .left_side_bearing(id)
            .ok_or(FontError::MissingGlyphGID(gid))?;
        let italics = self.italics(gid);
        let attachment = self.attachment(gid);
        Ok(Glyph {
            font: self,
            gid,
            bbox: (
                Unit::<FUnit>::new(x_min.into()),
                Unit::<FUnit>::new(y_min.into()),
                Unit::<FUnit>::new(x_max.into()),
                Unit::<FUnit>::new(y_max.into()),
            ),
            advance: Unit::<FUnit>::new(advance.into()),
            lsb: Unit::<FUnit>::new(lsb.into()),
            italics: Unit::<FUnit>::new(italics.into()),
            attachment: Unit::<FUnit>::new(attachment.into()),
        })
    }

    fn kern_for(
        &self,
        glyph_id: GlyphId,
        height: Unit<FUnit>,
        side: crate::font::kerning::Corner,
    ) -> Option<Unit<FUnit>> {
        use crate::font::kerning::Corner;
        let info = self.math.math_glyph_info().ok()?.math_kern_info()?.ok()?;
        let corner = match side {
            Corner::TopRight => MathKernCorner::TopRight,
            Corner::TopLeft => MathKernCorner::TopLeft,
            Corner::BottomRight => MathKernCorner::BottomRight,
            Corner::BottomLeft => MathKernCorner::BottomLeft,
        };
        let kern = info.kern(sk_gid(glyph_id), corner)?;
        // The spec's bands: the kern of the first correction height above
        // `height`, or the last kern above them all. The heights are whole
        // design units, so `floor` keeps `height < h` exact.
        let k = kern.kerning(height.unitless(FUnit).floor() as i32)?;
        Some(Unit::<FUnit>::new(f64::from(k)))
    }

    fn font_units_to_em(&self) -> Unit<Ratio<Em, FUnit>> {
        // Outlines and metrics are in design units for TrueType and CFF alike
        // (skrifa applies a CFF FontMatrix itself), so one em is `upem` units.
        Unit::<Ratio<Em, FUnit>>::new(f64::from(f32::from(self.units_per_em).recip()))
    }

    fn glyph_script_alternate(&self, gid: GlyphId, script_level: ScriptLevel) -> Option<GlyphId> {
        let gsub = self.gsub.as_ref()?;
        let level = if let ScriptLevel::LevelTwo = script_level { 1 } else { 0 };
        let features = gsub.feature_list().ok()?;
        let lookups = gsub.lookup_list().ok()?;
        let ssty = Tag::new(b"ssty");
        // The first `ssty` feature, as ttf-parser's `features.find` picked.
        let record = features
            .feature_records()
            .iter()
            .find(|r| r.feature_tag() == ssty)?;
        let feature = record.feature(features.offset_data()).ok()?;
        let target = sk_gid(gid);

        for index in feature.lookup_list_indices() {
            let Ok(lookup) = lookups.lookups().get(usize::from(index.get())) else {
                continue;
            };
            let Ok(subtables) = lookup.subtables() else {
                continue;
            };
            match subtables {
                SubstitutionSubtables::Single(subs) => {
                    for sub in subs.iter().flatten() {
                        if let Some(out) = single_substitute(&sub, target) {
                            return Some(out);
                        }
                    }
                }
                SubstitutionSubtables::Alternate(subs) => {
                    for sub in subs.iter().flatten() {
                        if let Some(out) = alternate(&sub, target, level) {
                            return Some(out);
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }
}

fn sk_gid(gid: GlyphId) -> skrifa::GlyphId {
    let id: u16 = gid.into();
    skrifa::GlyphId::new(u32::from(id))
}

fn to_gid(gid: u32) -> Option<GlyphId> {
    u16::try_from(gid).ok().map(GlyphId::from)
}

/// The glyph a single substitution maps `target` to.
fn single_substitute(sub: &SingleSubst<'_>, target: skrifa::GlyphId) -> Option<GlyphId> {
    match sub {
        SingleSubst::Format1(t) => {
            t.coverage().ok()?.get(target)?;
            let id = u16::try_from(target.to_u32()).ok()?;
            Some(GlyphId::from(id.wrapping_add_signed(t.delta_glyph_id())))
        }
        SingleSubst::Format2(t) => {
            let i = t.coverage().ok()?.get(target)?;
            let out = t.substitute_glyph_ids().get(i as usize)?.get();
            to_gid(u32::from(out.to_u16()))
        }
        SingleSubst::Format3(t) => {
            t.coverage().ok()?.get(target)?;
            let out = i64::from(target.to_u32()) + i64::from(t.delta_glyph_id().to_i32());
            to_gid(u32::try_from(out).ok()?)
        }
        SingleSubst::Format4(t) => {
            let i = t.coverage().ok()?.get(target)?;
            to_gid(t.substitute_glyph_ids().get(i as usize)?.get().to_u32())
        }
    }
}

/// The `level`-th alternate of `target` (clamped to the last one).
fn alternate(sub: &AlternateSubst<'_>, target: skrifa::GlyphId, level: usize) -> Option<GlyphId> {
    match sub {
        AlternateSubst::Format1(t) => {
            let i = t.coverage().ok()?.get(target)?;
            let set = t.alternate_sets().get(i as usize).ok()?;
            let ids = set.alternate_glyph_ids();
            let pick = ids.get(level.min(ids.len().checked_sub(1)?))?.get();
            to_gid(u32::from(pick.to_u16()))
        }
        AlternateSubst::Format2(t) => {
            let i = t.coverage().ok()?.get(target)?;
            let set = t.alternate_sets().get(i as usize).ok()?;
            let ids = set.alternate_glyph_ids();
            let pick = ids.get(level.min(ids.len().checked_sub(1)?))?.get();
            to_gid(pick.to_u32())
        }
    }
}

/// The extremes of every point drawn, control points included.
#[derive(Default)]
struct BBoxPen {
    seen: bool,
    x_min: f32,
    y_min: f32,
    x_max: f32,
    y_max: f32,
}

impl BBoxPen {
    fn add(&mut self, x: f32, y: f32) {
        if self.seen {
            self.x_min = self.x_min.min(x);
            self.y_min = self.y_min.min(y);
            self.x_max = self.x_max.max(x);
            self.y_max = self.y_max.max(y);
        } else {
            *self = BBoxPen { seen: true, x_min: x, y_min: y, x_max: x, y_max: y };
        }
    }
}

impl OutlinePen for BBoxPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.add(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.add(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.add(cx0, cy0);
        self.add(x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.add(cx0, cy0);
        self.add(cx1, cy1);
        self.add(x, y);
    }
    fn close(&mut self) {}
}

fn is_extender(part: &GlyphPartRecord) -> bool {
    part.part_flags()
        .contains(skrifa::raw::tables::math::PartFlags::EXTENDER_FLAG)
}

fn max_overlap(min_connector_overlap: u32, left: &GlyphPartRecord, right: &GlyphPartRecord) -> u32 {
    // NOTE: an adaptation of the corresponding code in the crate "font".
    let overlap = std::cmp::min(
        left.end_connector_length().to_u16(),
        right.start_connector_length().to_u16(),
    );
    let overlap = std::cmp::min(overlap, right.full_advance().to_u16() / 2);
    std::cmp::max(overlap.into(), min_connector_overlap)
}

fn construct_glyphs(
    min_connector_overlap: u32,
    parts: &[GlyphPartRecord],
    size: u32,
) -> Vec<GlyphInstruction> {
    let mut n_ext = 0;
    let mut n_nonext = 0;
    let mut size_ext: u32 = 0;
    let mut size_nonext: u32 = 0;
    for part in parts {
        if is_extender(part) {
            n_ext += 1;
            size_ext += u32::from(part.full_advance().to_u16());
        } else {
            n_nonext += 1;
            size_nonext += u32::from(part.full_advance().to_u16());
        }
    }

    // Determine whether we need extenders at all.
    let max_size_no_extender = size_nonext - (n_nonext - 1) * min_connector_overlap;
    let min_repeats = if max_size_no_extender >= size {
        0
    } else {
        let quotient = size_ext - n_ext * min_connector_overlap;
        let numerator = size - max_size_no_extender;
        // The fewest repeats for the extended glyph to exceed the size, rounded up.
        let min_repeats = numerator / quotient;
        if numerator.rem_euclid(quotient) != 0 {
            min_repeats + 1
        } else {
            min_repeats
        }
    };

    let size_without_overlap = size_nonext + size_ext * min_repeats;
    let min_overlap_total = (n_nonext + n_ext * min_repeats - 1) * min_connector_overlap;

    let mut max_overlap_total: u32 = 0;
    let mut prev: Option<&GlyphPartRecord> = None;
    for part in parts {
        if is_extender(part) {
            if min_repeats == 0 {
                continue;
            } else if min_repeats > 1 {
                // An extender repeated also overlaps itself.
                max_overlap_total += (min_repeats - 1) * max_overlap(min_connector_overlap, part, part);
            }
        }
        if let Some(prev) = prev {
            max_overlap_total += max_overlap(min_connector_overlap, prev, part);
        }
        prev = Some(part);
    }

    let size_with_min_overlap = size_without_overlap - min_overlap_total;
    let size_with_max_overlap = size_without_overlap - max_overlap_total;
    // TODO (upstream): Asana-Math.otf's min_connector_overlap is abnormally big.
    debug_assert!(size_with_min_overlap >= size);
    // Upstream note: in FiraMath, sizes between 4760 and 5400 can't be built,
    // so the overlap may exceed the maximum.

    // f such that size = (1 - f) * size_with_min_overlap + f * size_with_max_overlap
    let factor = f64::from(size_with_min_overlap - size)
        / f64::from(size_with_min_overlap - size_with_max_overlap);

    let mut instructions = Vec::with_capacity((n_nonext + min_repeats * n_ext) as usize);
    let mut prev_part: Option<&GlyphPartRecord> = None;
    for part in parts {
        let n_repeats = if is_extender(part) { min_repeats } else { 1 };
        for _ in 0..n_repeats {
            let overlap = if let Some(prev_part) = prev_part {
                let max_overlap = max_overlap(min_connector_overlap, prev_part, part);
                // Floor: under-estimate the overlap so the glyph is at least `size`.
                let overlap = min_connector_overlap
                    + ((factor * f64::from(max_overlap - min_connector_overlap)).floor() as u32);
                debug_assert!(overlap >= min_connector_overlap);
                overlap
            } else {
                0
            };
            instructions.push(GlyphInstruction {
                gid: GlyphId::from(part.glyph_id().to_u16()),
                overlap: overlap.try_into().unwrap(),
            });
            prev_part = Some(part);
        }
    }
    instructions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::MathFont;

    const FIRA_MATH_FONT_FILE: &[u8] = include_bytes!("../../../resources/FiraMath_Regular.otf");

    #[test]
    fn test_script_substitute() {
        let font = SkrifaMathFont::new(FIRA_MATH_FONT_FILE).unwrap();
        // Prime (U+2032) and its two script-size substitutes.
        let prime_glyph = GlyphId::from(0x22c_u16);
        assert_eq!(
            font.glyph_script_alternate(prime_glyph, ScriptLevel::LevelOne),
            Some(GlyphId::from(0x61f_u16)),
        );
        assert_eq!(
            font.glyph_script_alternate(prime_glyph, ScriptLevel::LevelTwo),
            Some(GlyphId::from(0x626_u16)),
        );
    }

    fn parts_for<'a>(font: &SkrifaMathFont<'a>, c: char, axis: StretchAxis) -> &'a [GlyphPartRecord] {
        let gid = font.glyph_index(c).unwrap();
        let variants = font.variants().unwrap();
        let construction = variants.glyph_construction(sk_gid(gid), axis).unwrap();
        construction.glyph_assembly().unwrap().unwrap().part_records()
    }

    fn size_instrs(instrs: Vec<GlyphInstruction>, parts: &[GlyphPartRecord]) -> u32 {
        let mut total: u32 = 0;
        for GlyphInstruction { gid, overlap } in instrs {
            // Crude: the advance of the part with that glyph id.
            let advance: u32 = parts
                .iter()
                .find(|p| GlyphId::from(p.glyph_id().to_u16()) == gid)
                .unwrap()
                .full_advance()
                .to_u16()
                .into();
            total += advance;
            total -= u32::from(overlap);
        }
        total
    }

    #[test]
    fn test_construct_glyphs() {
        let font = SkrifaMathFont::new(FIRA_MATH_FONT_FILE).unwrap();
        let min_overlap: u32 = font.variants().unwrap().min_connector_overlap().to_u16().into();
        // (glyph, axis, smallest buildable size) from FiraMath's tables.
        for (c, axis, min_size) in [
            ('}', StretchAxis::Vertical, 4500.),
            ('√', StretchAxis::Vertical, 5700.),
            ('⎴', StretchAxis::Horizontal, 6312.),
        ] {
            let parts = parts_for(&font, c, axis);
            let max_size = 25000.;
            let n_steps = 50;
            for i in 1..n_steps {
                let size = min_size + (max_size - min_size) * f64::from(i) / f64::from(n_steps - 1);
                let instrs = construct_glyphs(min_overlap, parts, size.ceil() as u32);
                let total = size_instrs(instrs, parts);
                assert!(f64::from(total) > size, "{c}: {total} <= {size}");
                assert!(f64::from(total) < 1.01 * size, "{c}: {total} too large for {size}");
            }
        }
    }
}
