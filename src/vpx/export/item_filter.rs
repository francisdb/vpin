//! Which game items a whole-table export includes.
//!
//! Both [`export_obj`](super::obj_export::export_obj) and
//! [`export_gltf`](super::gltf_export::export_gltf) walk the table's game
//! items in storage order and hand each one to their own writer. The
//! [`ItemFilter`] in their options decides, per item, whether the writer
//! sees it at all. How an included item is written (which parts, which
//! materials) stays with the writer.
//!
//! Two presets exist:
//!
//! - [`ItemFilter::everything`]: every item type that has geometry, skipping
//!   only the items that are not rendered. Both exporters' default.
//! - [`ItemFilter::vpinball_obj_export`]: what vpinball's
//!   `File -> Export -> OBJ Mesh` selects, declared in
//!   [`vpinball_rules`](super::vpinball_rules). The OBJ exporter's strict
//!   preset.
//!
//! Either can be narrowed or widened with the builder methods, so an OBJ
//! can carry every item and a glTF can carry only what vpinball's OBJ
//! export would.

use crate::vpx::compat;
use crate::vpx::gameitem::GameItemEnum;
use crate::vpx::version::Version;
use std::fmt;
use std::sync::Arc;

/// A game item type, as the exporters select it.
///
/// One variant per [`GameItemEnum`] variant except
/// [`Generic`](GameItemEnum::Generic), which is never exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemType {
    /// A wall, `Surface` in vpinball.
    Wall,
    /// A flipper.
    Flipper,
    /// A timer; has no geometry.
    Timer,
    /// A plunger.
    Plunger,
    /// A text box; has no geometry.
    TextBox,
    /// A bumper.
    Bumper,
    /// A trigger.
    Trigger,
    /// A light.
    Light,
    /// A kicker.
    Kicker,
    /// A decal.
    Decal,
    /// A gate.
    Gate,
    /// A spinner.
    Spinner,
    /// A ramp.
    Ramp,
    /// A reel; has no geometry.
    Reel,
    /// A light sequencer; has no geometry.
    LightSequencer,
    /// A primitive.
    Primitive,
    /// A flasher.
    Flasher,
    /// A rubber.
    Rubber,
    /// A hit target.
    HitTarget,
    /// A ball.
    Ball,
    /// A part group; has no geometry.
    PartGroup,
}

impl ItemType {
    /// Every item type, in vpinball's item type order.
    pub const ALL: [ItemType; 21] = [
        ItemType::Wall,
        ItemType::Flipper,
        ItemType::Timer,
        ItemType::Plunger,
        ItemType::TextBox,
        ItemType::Bumper,
        ItemType::Trigger,
        ItemType::Light,
        ItemType::Kicker,
        ItemType::Decal,
        ItemType::Gate,
        ItemType::Spinner,
        ItemType::Ramp,
        ItemType::Reel,
        ItemType::LightSequencer,
        ItemType::Primitive,
        ItemType::Flasher,
        ItemType::Rubber,
        ItemType::HitTarget,
        ItemType::Ball,
        ItemType::PartGroup,
    ];

    /// The item types that produce geometry in an export. Timers, text
    /// boxes, reels, light sequencers and part groups have no mesh.
    pub const WITH_GEOMETRY: [ItemType; 16] = [
        ItemType::Wall,
        ItemType::Flipper,
        ItemType::Plunger,
        ItemType::Bumper,
        ItemType::Trigger,
        ItemType::Light,
        ItemType::Kicker,
        ItemType::Decal,
        ItemType::Gate,
        ItemType::Spinner,
        ItemType::Ramp,
        ItemType::Primitive,
        ItemType::Flasher,
        ItemType::Rubber,
        ItemType::HitTarget,
        ItemType::Ball,
    ];

    /// The type of a game item, `None` for a
    /// [`Generic`](GameItemEnum::Generic) item.
    pub fn of(item: &GameItemEnum) -> Option<ItemType> {
        let item_type = match item {
            GameItemEnum::Wall(_) => ItemType::Wall,
            GameItemEnum::Flipper(_) => ItemType::Flipper,
            GameItemEnum::Timer(_) => ItemType::Timer,
            GameItemEnum::Plunger(_) => ItemType::Plunger,
            GameItemEnum::TextBox(_) => ItemType::TextBox,
            GameItemEnum::Bumper(_) => ItemType::Bumper,
            GameItemEnum::Trigger(_) => ItemType::Trigger,
            GameItemEnum::Light(_) => ItemType::Light,
            GameItemEnum::Kicker(_) => ItemType::Kicker,
            GameItemEnum::Decal(_) => ItemType::Decal,
            GameItemEnum::Gate(_) => ItemType::Gate,
            GameItemEnum::Spinner(_) => ItemType::Spinner,
            GameItemEnum::Ramp(_) => ItemType::Ramp,
            GameItemEnum::Reel(_) => ItemType::Reel,
            GameItemEnum::LightSequencer(_) => ItemType::LightSequencer,
            GameItemEnum::Primitive(_) => ItemType::Primitive,
            GameItemEnum::Flasher(_) => ItemType::Flasher,
            GameItemEnum::Rubber(_) => ItemType::Rubber,
            GameItemEnum::HitTarget(_) => ItemType::HitTarget,
            GameItemEnum::Ball(_) => ItemType::Ball,
            GameItemEnum::PartGroup(_) => ItemType::PartGroup,
            GameItemEnum::Generic(_, _) => return None,
        };
        Some(item_type)
    }

    /// The type name as [`GameItemEnum::type_name`] spells it.
    pub fn name(self) -> &'static str {
        match self {
            ItemType::Wall => "Wall",
            ItemType::Flipper => "Flipper",
            ItemType::Timer => "Timer",
            ItemType::Plunger => "Plunger",
            ItemType::TextBox => "TextBox",
            ItemType::Bumper => "Bumper",
            ItemType::Trigger => "Trigger",
            ItemType::Light => "Light",
            ItemType::Kicker => "Kicker",
            ItemType::Decal => "Decal",
            ItemType::Gate => "Gate",
            ItemType::Spinner => "Spinner",
            ItemType::Ramp => "Ramp",
            ItemType::Reel => "Reel",
            ItemType::LightSequencer => "LightSequencer",
            ItemType::Primitive => "Primitive",
            ItemType::Flasher => "Flasher",
            ItemType::Rubber => "Rubber",
            ItemType::HitTarget => "HitTarget",
            ItemType::Ball => "Ball",
            ItemType::PartGroup => "PartGroup",
        }
    }

    const fn bit(self) -> u32 {
        1 << (self as u32)
    }
}

/// A set of [`ItemType`]s.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ItemTypeSet(u32);

impl ItemTypeSet {
    /// The empty set.
    pub const EMPTY: ItemTypeSet = ItemTypeSet(0);

    /// The set holding exactly `types`.
    pub const fn of(types: &[ItemType]) -> ItemTypeSet {
        let mut bits = 0;
        let mut i = 0;
        while i < types.len() {
            bits |= types[i].bit();
            i += 1;
        }
        ItemTypeSet(bits)
    }

    /// Whether `item_type` is in the set.
    pub const fn contains(self, item_type: ItemType) -> bool {
        self.0 & item_type.bit() != 0
    }

    /// The set with `item_type` added.
    pub const fn with(self, item_type: ItemType) -> ItemTypeSet {
        ItemTypeSet(self.0 | item_type.bit())
    }

    /// The set with `item_type` removed.
    pub const fn without(self, item_type: ItemType) -> ItemTypeSet {
        ItemTypeSet(self.0 & !item_type.bit())
    }

    /// Whether the set holds no type.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The types in the set, in [`ItemType::ALL`] order.
    pub fn iter(self) -> impl Iterator<Item = ItemType> {
        ItemType::ALL.into_iter().filter(move |t| self.contains(*t))
    }
}

impl FromIterator<ItemType> for ItemTypeSet {
    fn from_iter<I: IntoIterator<Item = ItemType>>(iter: I) -> Self {
        iter.into_iter()
            .fold(ItemTypeSet::EMPTY, |set, t| set.with(t))
    }
}

impl fmt::Debug for ItemTypeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set()
            .entries(self.iter().map(ItemType::name))
            .finish()
    }
}

/// A caller supplied test on a game item, see [`ItemFilter::with_predicate`].
pub type ItemPredicate = Arc<dyn Fn(&GameItemEnum) -> bool + Send + Sync>;

/// Selects the game items a whole-table export includes.
///
/// An item is included when every rule passes:
///
/// 1. its type is in [`types`](Self::types);
/// 2. it is rendered, unless [`include_invisible`](Self::include_invisible)
///    is set or its type is in
///    [`ignore_visible_flag`](Self::ignore_visible_flag);
/// 3. it is shown in the editor, when
///    [`skip_editor_hidden`](Self::skip_editor_hidden) is set;
/// 4. it is not part of the desktop backdrop, when
///    [`skip_backdrop`](Self::skip_backdrop) is set;
/// 5. its name passes [`only_names`](Self::only_names) and
///    [`exclude_names`](Self::exclude_names);
/// 6. the [`predicate`](Self::with_predicate), when set, returns `true`.
///
/// Start from a preset and adjust it:
///
/// ```
/// use vpin::vpx::export::item_filter::{ItemFilter, ItemType};
///
/// // A glTF with only what vpinball's OBJ export would carry, plus lights
/// let filter = ItemFilter::vpinball_obj_export().with_type(ItemType::Light);
///
/// // Everything except the items of two hidden editor layers, by name
/// let filter = ItemFilter::everything()
///     .skip_editor_hidden(true)
///     .exclude_names(["Wall12", "Wall13"]);
/// ```
#[derive(Clone)]
pub struct ItemFilter {
    types: ItemTypeSet,
    include_invisible: bool,
    ignore_visible_flag: ItemTypeSet,
    skip_editor_hidden: bool,
    skip_backdrop: bool,
    only_names: Option<Vec<String>>,
    exclude_names: Vec<String>,
    predicate: Option<ItemPredicate>,
}

impl ItemFilter {
    /// Every item type with geometry, skipping the items that are not
    /// rendered at play time.
    ///
    /// Lights are the exception to the visibility rule: they are included
    /// whatever their visible flag says, so the glTF exporter can mark them
    /// with `KHR_node_visibility` instead of dropping them.
    pub fn everything() -> Self {
        Self {
            types: ItemTypeSet::of(&ItemType::WITH_GEOMETRY),
            include_invisible: false,
            ignore_visible_flag: ItemTypeSet::of(&[ItemType::Light]),
            skip_editor_hidden: false,
            skip_backdrop: false,
            only_names: None,
            exclude_names: Vec::new(),
            predicate: None,
        }
    }

    /// What vpinball's `File -> Export -> OBJ Mesh` selects. See
    /// [`vpinball_rules::obj_export_filter`](super::vpinball_rules::obj_export_filter).
    pub fn vpinball_obj_export() -> Self {
        super::vpinball_rules::obj_export_filter()
    }

    /// The types to include.
    pub fn types(&self) -> ItemTypeSet {
        self.types
    }

    /// Replace the types to include.
    pub fn with_types(mut self, types: ItemTypeSet) -> Self {
        self.types = types;
        self
    }

    /// Add a type to include.
    pub fn with_type(mut self, item_type: ItemType) -> Self {
        self.types = self.types.with(item_type);
        self
    }

    /// Remove a type from the ones to include.
    pub fn without_type(mut self, item_type: ItemType) -> Self {
        self.types = self.types.without(item_type);
        self
    }

    /// Whether items that are not rendered are included.
    pub fn includes_invisible(&self) -> bool {
        self.include_invisible
    }

    /// Include items that are not rendered at play time. The glTF exporter
    /// marks them with `KHR_node_visibility`; the OBJ exporter writes them
    /// like any other item.
    pub fn include_invisible(mut self, include: bool) -> Self {
        self.include_invisible = include;
        self
    }

    /// The types included whatever their visible flag says.
    pub fn ignored_visible_flags(&self) -> ItemTypeSet {
        self.ignore_visible_flag
    }

    /// Replace the types included whatever their visible flag says.
    pub fn ignore_visible_flag(mut self, types: ItemTypeSet) -> Self {
        self.ignore_visible_flag = types;
        self
    }

    /// Whether items hidden in the editor are skipped.
    pub fn skips_editor_hidden(&self) -> bool {
        self.skip_editor_hidden
    }

    /// Skip items hidden in the editor: items whose own `LVIS` record is
    /// `false`, which is the 10.7 layer visibility stored per item. An
    /// absent record counts as shown. The flag has no effect on the game,
    /// so an item hidden this way is still part of the table.
    pub fn skip_editor_hidden(mut self, skip: bool) -> Self {
        self.skip_editor_hidden = skip;
        self
    }

    /// Whether items that are part of the desktop backdrop are skipped.
    pub fn skips_backdrop(&self) -> bool {
        self.skip_backdrop
    }

    /// Skip items flagged as part of the desktop backdrop (the `BGLS`
    /// record of lights, decals, flashers, timers, light sequencers and
    /// part groups).
    pub fn skip_backdrop(mut self, skip: bool) -> Self {
        self.skip_backdrop = skip;
        self
    }

    /// Include only the items with one of these names. Names compare case
    /// insensitively, like vpinball treats them.
    pub fn only_names<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.only_names = Some(names.into_iter().map(|n| fold_name(n.as_ref())).collect());
        self
    }

    /// Exclude the items with one of these names. Names compare case
    /// insensitively, like vpinball treats them.
    pub fn exclude_names<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.exclude_names
            .extend(names.into_iter().map(|n| fold_name(n.as_ref())));
        self
    }

    /// Add a test of your own. It runs last, only for items that passed the
    /// other rules.
    pub fn with_predicate<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&GameItemEnum) -> bool + Send + Sync + 'static,
    {
        self.predicate = Some(Arc::new(predicate));
        self
    }

    /// Whether an export with this filter includes `item`. `version` is the
    /// table's file version, which decides how some visible flags are read.
    pub fn includes(&self, item: &GameItemEnum, version: &Version) -> bool {
        let Some(item_type) = ItemType::of(item) else {
            return false;
        };
        if !self.types.contains(item_type) {
            return false;
        }
        if !self.include_invisible
            && !self.ignore_visible_flag.contains(item_type)
            && is_rendered(item, version) == Some(false)
        {
            return false;
        }
        if self.skip_editor_hidden && item.editor_layer_visibility() == Some(false) {
            return false;
        }
        if self.skip_backdrop && is_backdrop(item) {
            return false;
        }
        if self.only_names.is_some() || !self.exclude_names.is_empty() {
            let name = fold_name(item.name());
            if let Some(only) = &self.only_names
                && !only.contains(&name)
            {
                return false;
            }
            if self.exclude_names.contains(&name) {
                return false;
            }
        }
        if let Some(predicate) = &self.predicate
            && !predicate(item)
        {
            return false;
        }
        true
    }
}

impl fmt::Debug for ItemFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ItemFilter")
            .field("types", &self.types)
            .field("include_invisible", &self.include_invisible)
            .field("ignore_visible_flag", &self.ignore_visible_flag)
            .field("skip_editor_hidden", &self.skip_editor_hidden)
            .field("skip_backdrop", &self.skip_backdrop)
            .field("only_names", &self.only_names)
            .field("exclude_names", &self.exclude_names)
            .field("predicate", &self.predicate.as_ref().map(|_| "<fn>"))
            .finish()
    }
}

fn fold_name(name: &str) -> String {
    name.to_lowercase()
}

/// Whether the item is drawn at play time, `None` for the types that have
/// no item level visible flag: bumpers and kickers decide per part or per
/// kicker type, decals and balls are always drawn, and the rest have no
/// geometry.
fn is_rendered(item: &GameItemEnum, version: &Version) -> Option<bool> {
    let rendered = match item {
        GameItemEnum::Primitive(primitive) => compat::primitive_is_visible(primitive, version),
        GameItemEnum::Wall(wall) => wall.is_top_bottom_visible || wall.is_side_visible,
        GameItemEnum::Ramp(ramp) => ramp.is_visible,
        GameItemEnum::Rubber(rubber) => rubber.is_visible,
        GameItemEnum::Flasher(flasher) => flasher.is_visible,
        GameItemEnum::Flipper(flipper) => flipper.is_visible,
        GameItemEnum::Spinner(spinner) => spinner.is_visible,
        GameItemEnum::HitTarget(hit_target) => hit_target.is_visible,
        GameItemEnum::Gate(gate) => gate.is_visible,
        GameItemEnum::Trigger(trigger) => trigger.is_visible,
        GameItemEnum::Plunger(plunger) => plunger.is_visible,
        GameItemEnum::Light(light) => light.visible.unwrap_or(true),
        GameItemEnum::Bumper(_)
        | GameItemEnum::Kicker(_)
        | GameItemEnum::Decal(_)
        | GameItemEnum::Ball(_)
        | GameItemEnum::Timer(_)
        | GameItemEnum::TextBox(_)
        | GameItemEnum::Reel(_)
        | GameItemEnum::LightSequencer(_)
        | GameItemEnum::PartGroup(_)
        | GameItemEnum::Generic(_, _) => return None,
    };
    Some(rendered)
}

/// Whether the item is part of the desktop backdrop, vpinball's
/// `m_desktopBackdrop`, stored in the `BGLS` record of the types that have
/// one.
fn is_backdrop(item: &GameItemEnum) -> bool {
    match item {
        GameItemEnum::Light(light) => light.is_backglass,
        GameItemEnum::Decal(decal) => decal.backglass,
        GameItemEnum::Flasher(flasher) => flasher.backglass.unwrap_or(false),
        GameItemEnum::Timer(timer) => timer.backglass,
        GameItemEnum::LightSequencer(sequencer) => sequencer.backglass,
        GameItemEnum::PartGroup(group) => group.backglass,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vpx::gameitem::ball::Ball;
    use crate::vpx::gameitem::decal::Decal;
    use crate::vpx::gameitem::flipper::Flipper;
    use crate::vpx::gameitem::light::Light;
    use crate::vpx::gameitem::ramp::Ramp;
    use crate::vpx::gameitem::timer::Timer;

    fn version() -> Version {
        Version::new(1080)
    }

    fn flipper(name: &str, is_visible: bool) -> GameItemEnum {
        GameItemEnum::Flipper(Flipper {
            name: name.to_string(),
            is_visible,
            ..Flipper::default()
        })
    }

    fn ramp(name: &str, is_visible: bool) -> GameItemEnum {
        GameItemEnum::Ramp(Ramp {
            name: name.to_string(),
            is_visible,
            ..Ramp::default()
        })
    }

    #[test]
    fn type_set_holds_exactly_the_given_types() {
        let set = ItemTypeSet::of(&[ItemType::Wall, ItemType::Ball]);
        assert!(set.contains(ItemType::Wall));
        assert!(set.contains(ItemType::Ball));
        assert!(!set.contains(ItemType::Light));
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            [ItemType::Wall, ItemType::Ball]
        );
        assert_eq!(
            set.with(ItemType::Light).without(ItemType::Wall),
            ItemTypeSet::of(&[ItemType::Light, ItemType::Ball])
        );
        assert!(ItemTypeSet::EMPTY.is_empty());
        assert_eq!(format!("{set:?}"), r#"{"Wall", "Ball"}"#);
    }

    #[test]
    fn every_type_maps_to_a_distinct_bit() {
        let all: ItemTypeSet = ItemType::ALL.into_iter().collect();
        assert_eq!(all.iter().count(), ItemType::ALL.len());
        for t in ItemType::ALL {
            assert_eq!(ItemTypeSet::of(&[t]).iter().collect::<Vec<_>>(), [t]);
        }
    }

    #[test]
    fn everything_skips_types_without_geometry_and_generic_items() {
        let filter = ItemFilter::everything();
        assert!(!filter.includes(&GameItemEnum::Timer(Timer::default()), &version()));
        let generic = GameItemEnum::Generic(
            999,
            crate::vpx::gameitem::generic::Generic {
                name: "g".to_string(),
                fields: Vec::new(),
            },
        );
        assert!(!filter.includes(&generic, &version()));
        assert!(filter.includes(&flipper("f", true), &version()));
    }

    #[test]
    fn invisible_items_are_skipped_unless_asked_for() {
        let filter = ItemFilter::everything();
        assert!(!filter.includes(&ramp("r", false), &version()));
        assert!(
            filter
                .clone()
                .include_invisible(true)
                .includes(&ramp("r", false), &version())
        );
        assert!(
            filter
                .ignore_visible_flag(ItemTypeSet::of(&[ItemType::Ramp]))
                .includes(&ramp("r", false), &version())
        );
    }

    #[test]
    fn invisible_lights_are_included_by_everything() {
        let light = GameItemEnum::Light(Light {
            visible: Some(false),
            ..Light::default()
        });
        assert!(ItemFilter::everything().includes(&light, &version()));
    }

    #[test]
    fn editor_hidden_items_are_skipped_only_on_request() {
        let mut hidden = flipper("f", true);
        hidden.set_editor_layer_visibility(Some(false));
        let filter = ItemFilter::everything();
        assert!(filter.includes(&hidden, &version()));
        let filter = filter.skip_editor_hidden(true);
        assert!(!filter.includes(&hidden, &version()));
        // An absent record counts as shown
        assert!(filter.includes(&flipper("f", true), &version()));
        let mut shown = flipper("f", true);
        shown.set_editor_layer_visibility(Some(true));
        assert!(filter.includes(&shown, &version()));
    }

    #[test]
    fn backdrop_items_are_skipped_on_request() {
        let decal = GameItemEnum::Decal(Decal {
            backglass: true,
            ..Decal::default()
        });
        assert!(ItemFilter::everything().includes(&decal, &version()));
        assert!(
            !ItemFilter::everything()
                .skip_backdrop(true)
                .includes(&decal, &version())
        );
    }

    #[test]
    fn names_filter_case_insensitively() {
        let filter = ItemFilter::everything().only_names(["LeftFlipper"]);
        assert!(filter.includes(&flipper("leftflipper", true), &version()));
        assert!(!filter.includes(&flipper("RightFlipper", true), &version()));

        let filter = ItemFilter::everything().exclude_names(["leftflipper"]);
        assert!(!filter.includes(&flipper("LeftFlipper", true), &version()));
        assert!(filter.includes(&flipper("RightFlipper", true), &version()));
    }

    #[test]
    fn predicate_runs_last() {
        let filter =
            ItemFilter::everything().with_predicate(|item| item.name().starts_with("keep"));
        assert!(filter.includes(&flipper("keep_me", true), &version()));
        assert!(!filter.includes(&flipper("drop_me", true), &version()));
        // The predicate does not resurrect an item the other rules dropped
        assert!(!filter.includes(&ramp("keep_hidden", false), &version()));
    }

    #[test]
    fn type_rule_applies_before_anything_else() {
        let ball = GameItemEnum::Ball(Ball::default());
        assert!(ItemFilter::everything().includes(&ball, &version()));
        assert!(
            !ItemFilter::everything()
                .without_type(ItemType::Ball)
                .includes(&ball, &version())
        );
    }
}
