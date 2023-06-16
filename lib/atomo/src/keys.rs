use crate::batch::BoxedVec;

pub type ImKeyCollection = im::HashSet<BoxedVec, fxhash::FxBuildHasher>;

pub type MaybeImKeyCollection = Option<ImKeyCollection>;

#[derive(Default)]
pub struct VerticalKeys(Vec<MaybeImKeyCollection>);
