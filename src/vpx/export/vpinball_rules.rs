//! What vpinball's `File -> Export -> OBJ Mesh` selects, as filter data.
//!
//! This is the one place that spells out the selection rules of vpinball's
//! own exporter. The OBJ writer's parity quirks (combined wall blocks,
//! material lines, axis convention) live with the OBJ writer; only the
//! question "is this item exported at all" is answered here, so a glTF
//! export can apply the same selection.
//!
//! Source: `WinEditor::ExportTableMesh` in vpinball's
//! `src/ui/win/WinEditor.cpp`:
//!
//! ```cpp
//! ptCur->ExportMesh(loader);
//! for (const auto pedit : ptCur->GetParts())
//!    if (pedit->IsUIVisible(false) && pedit->m_desktopBackdrop == m_desktopBackdropView)
//!       pedit->ExportMesh(loader);
//! ```
//!
//! - The playfield quad always comes first; the exporters emit it
//!   themselves, it is not a game item.
//! - `m_desktopBackdropView` is the editor's current view; the export is
//!   made from the desktop view, so backdrop items are skipped.
//! - `ExportMesh` is a no-op for the item types without an implementation,
//!   and some implementations ignore the item's visible flag.
//! - `IsUIVisible(false)` is the item's own `m_uiVisible`, the `LVIS`
//!   record, without the part group hierarchy applied. This gate is *not*
//!   part of [`obj_export_filter`]: the record holds whatever the layer
//!   panel showed when the table was last saved, and released tables often
//!   carry `false` on most of their items, so honoring it drops nearly the
//!   whole table. Opt in with [`ItemFilter::skip_editor_hidden`] to match
//!   vpinball on a table whose layers are set up for it.

use super::item_filter::{ItemFilter, ItemType, ItemTypeSet};

/// The item types with an `ExportMesh` implementation in vpinball, so the
/// ones its OBJ export writes. Lights, decals, plungers, flashers, balls,
/// reels and the rest are left out.
pub const OBJ_EXPORT_TYPES: ItemTypeSet = ItemTypeSet::of(&[
    ItemType::Primitive, // Primitive::ExportMesh
    ItemType::Wall,      // Surface::ExportMesh
    ItemType::Ramp,      // Ramp::ExportMesh
    ItemType::Rubber,    // Rubber::ExportMesh
    ItemType::Bumper,    // Bumper::ExportMesh
    ItemType::Flipper,   // Flipper::ExportMesh
    ItemType::Gate,      // Gate::ExportMesh
    ItemType::Kicker,    // Kicker::ExportMesh
    ItemType::Spinner,   // Spinner::ExportMesh
    ItemType::HitTarget, // HitTarget::ExportMesh
    ItemType::Trigger,   // Trigger::ExportMesh
]);

/// The item types whose `ExportMesh` has no `m_d.m_visible` guard: vpinball
/// writes them even when they are invisible at play time.
pub const OBJ_EXPORT_IGNORES_VISIBLE_FLAG: ItemTypeSet = ItemTypeSet::of(&[
    ItemType::Flipper,   // flipper.cpp
    ItemType::Gate,      // gate.cpp
    ItemType::Spinner,   // spinner.cpp
    ItemType::HitTarget, // hittarget.cpp
]);

/// The filter that selects what vpinball's `File -> Export -> OBJ Mesh`
/// writes: [`OBJ_EXPORT_TYPES`], skipping items that are invisible at play
/// time except for [`OBJ_EXPORT_IGNORES_VISIBLE_FLAG`], and backdrop items.
///
/// The editor visibility gate is left off, see the module docs; add
/// [`ItemFilter::skip_editor_hidden`] for it.
pub fn obj_export_filter() -> ItemFilter {
    ItemFilter::everything()
        .with_types(OBJ_EXPORT_TYPES)
        .include_invisible(false)
        .ignore_visible_flag(OBJ_EXPORT_IGNORES_VISIBLE_FLAG)
        .skip_editor_hidden(false)
        .skip_backdrop(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::GameItemEnum;
    use crate::vpx::gameitem::flipper::Flipper;
    use crate::vpx::gameitem::light::Light;
    use crate::vpx::gameitem::plunger::Plunger;
    use crate::vpx::gameitem::ramp::Ramp;
    use crate::vpx::version::Version;

    fn version() -> Version {
        Version::new(1080)
    }

    #[test]
    fn selects_the_export_mesh_types_only() {
        let filter = obj_export_filter();
        assert!(filter.includes(&GameItemEnum::Ramp(Ramp::default()), &version()));
        assert!(!filter.includes(&GameItemEnum::Light(Light::default()), &version()));
        assert!(!filter.includes(&GameItemEnum::Plunger(Plunger::default()), &version()));
    }

    #[test]
    fn invisible_flippers_are_exported_but_invisible_ramps_are_not() {
        let filter = obj_export_filter();
        let flipper = GameItemEnum::Flipper(Flipper {
            is_visible: false,
            ..Flipper::default()
        });
        assert!(filter.includes(&flipper, &version()));
        let ramp = GameItemEnum::Ramp(Ramp {
            is_visible: false,
            ..Ramp::default()
        });
        assert!(!filter.includes(&ramp, &version()));
    }

    #[test]
    fn editor_hidden_items_are_skipped_only_on_request() {
        let mut ramp = GameItemEnum::Ramp(Ramp::default());
        ramp.set_editor_layer_visibility(Some(false));
        assert!(obj_export_filter().includes(&ramp, &version()));
        assert!(
            !obj_export_filter()
                .skip_editor_hidden(true)
                .includes(&ramp, &version())
        );
    }
}
