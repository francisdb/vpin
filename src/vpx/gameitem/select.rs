use crate::vpx::biff;

// TODO create the read side of this trait

pub trait WriteSharedAttributes {
    fn write_shared_attributes(&self, writer: &mut biff::BiffWriter);
}

/// Required trait for any type that has shared attributes
///
/// TODO we could create a macro to implement this trait for all types that have shared attributes.
/// TODO we could ise a shared struct to implement this trait for all types that have shared attributes. (composition)
pub trait HasSharedAttributes {
    fn is_locked(&self) -> bool;
    fn editor_layer(&self) -> u32;
    fn editor_layer_name(&self) -> Option<&str>;
    fn editor_layer_visibility(&self) -> Option<bool>;
    fn part_group_name(&self) -> Option<&str>;
}

impl<T> WriteSharedAttributes for T
where
    T: HasSharedAttributes,
{
    /// shared, order changed when part_group_name was introduced somewhere in 10.8.1 release cycle
    fn write_shared_attributes(&self, writer: &mut biff::BiffWriter) {
        writer.write_tagged_bool("LOCK", self.is_locked());

        // Different ordering based on part_group_name presence
        if let Some(group_name) = self.part_group_name() {
            if let Some(visibility) = self.editor_layer_visibility() {
                writer.write_tagged_bool("LVIS", visibility);
            }
            // will be removed in a future version
            writer.write_tagged_u32("LAYR", self.editor_layer());
            // will be removed in a future version
            if let Some(name) = self.editor_layer_name() {
                writer.write_tagged_string("LANR", name);
            }
            writer.write_tagged_string("GRUP", group_name);
        } else {
            writer.write_tagged_u32("LAYR", self.editor_layer());
            if let Some(name) = self.editor_layer_name() {
                writer.write_tagged_string("LANR", name);
            }
            if let Some(visibility) = self.editor_layer_visibility() {
                writer.write_tagged_bool("LVIS", visibility);
            }
        }
    }
}
