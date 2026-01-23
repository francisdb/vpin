use crate::vpx::biff;

// TODO create the read side of this trait

pub trait WriteSharedAttributes {
    fn write_shared_attributes(&self, writer: &mut biff::BiffWriter);
    fn read_shared_attribute(&mut self, tag_str: &str, reader: &mut biff::BiffReader) -> bool;
}

/// Required trait for any type that has shared attributes
///
/// TODO we could use a shared struct to implement this trait for all types that have shared attributes. (composition)
pub trait HasSharedAttributes {
    fn name(&self) -> &str;
    fn is_locked(&self) -> bool;
    /// in 10.8.1 Deprecated and replaced by part groups
    fn editor_layer(&self) -> Option<u32>;
    fn editor_layer_name(&self) -> Option<&str>;
    fn editor_layer_visibility(&self) -> Option<bool>;
    fn part_group_name(&self) -> Option<&str>;

    fn set_is_locked(&mut self, locked: bool);
    /// in 10.8.1 Deprecated and replaced by part groups
    fn set_editor_layer(&mut self, layer: Option<u32>);
    fn set_editor_layer_name(&mut self, name: Option<String>);
    fn set_editor_layer_visibility(&mut self, visibility: Option<bool>);
    fn set_part_group_name(&mut self, name: Option<String>);
}

pub trait TimerDataRoot {
    fn is_timer_enabled(&self) -> bool;
    fn timer_interval(&self) -> i32;
}

impl<T> WriteSharedAttributes for T
where
    T: HasSharedAttributes,
{
    fn write_shared_attributes(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged_bool("LOCK", self.is_locked());
        if let Some(layer) = self.editor_layer() {
            writer.write_tagged_u32("LAYR", layer);
        }
        if let Some(name) = self.editor_layer_name() {
            writer.write_tagged_string("LANR", name);
        }
        if let Some(group_name) = self.part_group_name() {
            writer.write_tagged_string("GRUP", group_name);
        }
        if let Some(visibility) = self.editor_layer_visibility() {
            writer.write_tagged_bool("LVIS", visibility);
        }
    }

    fn read_shared_attribute(&mut self, tag: &str, reader: &mut biff::BiffReader) -> bool {
        match tag {
            "LOCK" => {
                self.set_is_locked(reader.get_bool());
                true
            }
            "LAYR" => {
                self.set_editor_layer(Some(reader.get_u32()));
                true
            }
            "LANR" => {
                self.set_editor_layer_name(Some(reader.get_string()));
                true
            }
            "LVIS" => {
                self.set_editor_layer_visibility(Some(reader.get_bool()));
                true
            }
            "GRUP" => {
                self.set_part_group_name(Some(reader.get_string()));
                true
            }
            _ => false,
        }
    }
}

/// Macro to implement HasSharedAttributes for a given type
/// Assumes the type has the following fields:
/// - name: `String`
/// - is_locked: `bool`
/// - editor_layer: `Option<u32>`
/// - editor_layer_name: `Option<String>`
/// - editor_layer_visibility: `Option<bool>`
/// - part_group_name: `Option<String>`
#[macro_export]
macro_rules! impl_shared_attributes {
    ($ty:ty) => {
        impl $crate::vpx::gameitem::select::HasSharedAttributes for $ty {
            fn name(&self) -> &str {
                &self.name
            }
            fn is_locked(&self) -> bool {
                self.is_locked
            }
            fn editor_layer(&self) -> Option<u32> {
                self.editor_layer
            }
            fn editor_layer_name(&self) -> Option<&str> {
                self.editor_layer_name.as_deref()
            }
            fn editor_layer_visibility(&self) -> Option<bool> {
                self.editor_layer_visibility
            }
            fn part_group_name(&self) -> Option<&str> {
                self.part_group_name.as_deref()
            }

            fn set_is_locked(&mut self, locked: bool) {
                self.is_locked = locked;
            }
            fn set_editor_layer(&mut self, layer: Option<u32>) {
                self.editor_layer = layer;
            }
            fn set_editor_layer_name(&mut self, name: Option<String>) {
                self.editor_layer_name = name;
            }
            fn set_editor_layer_visibility(&mut self, visibility: Option<bool>) {
                self.editor_layer_visibility = visibility;
            }
            fn set_part_group_name(&mut self, name: Option<String>) {
                self.part_group_name = name;
            }
        }
    };
}
